//! Path rules, content identity, eligibility order, and the write transaction.
//!
//! The lexical table, the identity vector, and the classifier are pure. The
//! descriptor-walk and replace cases run against a private `TempDir`, which is
//! the only way to prove `O_NOFOLLOW`, mode preservation, and temporary-file
//! removal at all.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::ai_command_runner::WriterCoverage;
use crate::web::remote_control_api::dto::{TextEditBlockedReason, TextExpectation};

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, bytes).unwrap();
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn remote_text_edit_lexical_rules_refuse_before_any_filesystem_access() {
    for ok in [
        "a",
        "src/lib.rs",
        ".hidden",
        "sub/.conflux-inflight",
        "a..b/c",
    ] {
        assert!(validate_path(ok).is_ok(), "{ok}");
    }
    let long = "a".repeat(MAX_PATH_BYTES + 1);
    for bad in [
        "",
        "/etc/passwd",
        "a\\b",
        "a//b",
        "a/",
        "./a",
        "a/./b",
        "../a",
        "a/../b",
        "a\0b",
        long.as_str(),
    ] {
        assert!(validate_path(bad).is_err(), "{bad:?} must be refused");
    }
    assert!(validate_path(&"a".repeat(MAX_PATH_BYTES)).is_ok());
}

#[test]
fn remote_text_edit_content_identity_matches_the_published_vector() {
    assert_eq!(
        content_id(b"hello\n"),
        "sha256:5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
    );
    assert!(is_content_id(&content_id(b"")));
    for bad in [
        "sha256:",
        "sha1:5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03",
        "sha256:5891B5B522D5DF086D0FF0B110FBD9D21BB4FC7163AF34D08286A2E846F6BE03",
        "sha256:5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be0",
    ] {
        assert!(!is_content_id(bad), "{bad}");
    }
}

#[test]
fn remote_text_edit_containment_reasons_are_typed_and_ordered() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(root, "plain.txt", b"text\n");
    write(root, ".git", b"gitdir: elsewhere\n");
    write(root, ".wt/setup", b"echo\n");
    write(root, ".conflux-inflight", b"x\n");
    write(root, "sub/.conflux-inflight", b"y\n");
    write(root, "binary.bin", &[0xff, 0xfe]);
    write(root, "nul.txt", b"a\0b");
    write(root, "big.txt", &vec![b'a'; MAX_TEXT_BYTES as usize + 1]);
    write(root, "exec.sh", b"echo\n");
    std::fs::set_permissions(root.join("exec.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::create_dir(root.join("a-dir")).unwrap();
    std::os::unix::fs::symlink(root.join("plain.txt"), root.join("link.txt")).unwrap();
    std::os::unix::fs::symlink(root.join("sub"), root.join("linkdir")).unwrap();

    let reason = |path: &str| match read_text(root, path) {
        Err(TextFailure::NotEditable(reason)) => Some(reason),
        Ok(_) => None,
        Err(other) => panic!("{path}: unexpected {other:?}"),
    };
    assert_eq!(reason(".git"), Some(NotEditable::GitMetadata));
    assert_eq!(reason("sub/.GIT/config"), Some(NotEditable::GitMetadata));
    assert_eq!(reason(".wt/setup"), Some(NotEditable::ConfluxOwnedPath));
    assert_eq!(reason(".wt"), Some(NotEditable::ConfluxOwnedPath));
    assert_eq!(
        reason(".conflux-inflight"),
        Some(NotEditable::ConfluxOwnedPath)
    );
    assert_eq!(reason("sub/.conflux-inflight"), None);
    assert_eq!(reason("link.txt"), Some(NotEditable::UnsafePath));
    assert_eq!(
        reason("linkdir/.conflux-inflight"),
        Some(NotEditable::UnsafePath)
    );
    assert_eq!(reason("plain.txt/child"), Some(NotEditable::UnsafePath));
    assert_eq!(reason("a-dir"), Some(NotEditable::NotRegularFile));
    assert_eq!(reason("exec.sh"), Some(NotEditable::Executable));
    assert_eq!(reason("big.txt"), Some(NotEditable::TooLarge));
    assert_eq!(reason("binary.bin"), Some(NotEditable::NotText));
    assert_eq!(reason("nul.txt"), Some(NotEditable::NotText));
    assert_eq!(
        read_text(root, "missing/file.txt"),
        Err(TextFailure::MissingDirectory)
    );
    assert_eq!(
        read_text(&root.join("no-such-root"), "plain.txt"),
        Err(TextFailure::NotEditable(NotEditable::RootMissing))
    );
    assert_eq!(
        read_text(&root.join("linkdir"), "x"),
        Err(TextFailure::NotEditable(NotEditable::RootMissing)),
        "a symlinked root is not a root"
    );

    let absent = read_text(root, "new.txt").unwrap();
    assert_eq!(absent.content, None);
    let present = read_text(root, "plain.txt").unwrap();
    assert_eq!(present.content.as_deref(), Some("text\n"));
}

#[test]
fn remote_text_edit_nested_repository_probe_stops_the_walk() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    // The root's own `.git` is expected and never examined.
    std::fs::create_dir(root.join(".git")).unwrap();
    write(root, "top.txt", b"top\n");
    write(root, "wt/.git", b"gitdir: /elsewhere\n");
    write(root, "wt/src/lib.rs", b"fn main() {}\n");
    write(root, "sub/embedded/.git/HEAD", b"ref: refs/heads/main\n");
    write(root, "sub/embedded/file.txt", b"e\n");
    std::fs::create_dir_all(root.join("linked")).unwrap();
    std::os::unix::fs::symlink("/nonexistent", root.join("linked/.git")).unwrap();
    write(root, "linked/file.txt", b"l\n");

    assert!(read_text(root, "top.txt").is_ok());
    for path in ["wt/src/lib.rs", "sub/embedded/file.txt", "linked/file.txt"] {
        assert_eq!(
            read_text(root, path),
            Err(TextFailure::NotEditable(NotEditable::NestedRepository)),
            "{path}"
        );
        let before = std::fs::read(root.join(path)).unwrap();
        assert!(matches!(
            write_text(
                root,
                path,
                &TextExpectation::Content {
                    content_id: content_id(&before)
                },
                b"x"
            ),
            WriteSettlement::Refused(TextFailure::NotEditable(NotEditable::NestedRepository))
        ));
        assert_eq!(std::fs::read(root.join(path)).unwrap(), before);
    }
    // Read through the nested worktree's own root, the same file is served.
    assert!(read_text(&root.join("wt"), "src/lib.rs").is_ok());
}

#[test]
fn remote_text_edit_write_preserves_mode_and_leaves_no_temporary_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(root, "secret.txt", b"old\n");
    std::fs::set_permissions(
        root.join("secret.txt"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let settlement = write_text(
        root,
        "secret.txt",
        &TextExpectation::Content {
            content_id: content_id(b"old\n"),
        },
        b"new\n",
    );
    match settlement {
        WriteSettlement::Written {
            before,
            after,
            bytes,
            mode,
        } => {
            assert_eq!(before, Some(content_id(b"old\n")));
            assert_eq!(after, content_id(b"new\n"));
            assert_eq!(bytes, 4);
            assert_eq!(mode, 0o600);
        }
        other => panic!("expected a write, got {other:?}"),
    }
    assert_eq!(std::fs::read(root.join("secret.txt")).unwrap(), b"new\n");
    let mode = std::fs::metadata(root.join("secret.txt"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(listing(root), vec!["secret.txt".to_string()]);
}

#[test]
fn remote_text_edit_forced_failure_removes_the_temporary_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(root, "doc.md", b"before\n");

    let settlement = write_text_inner(
        root,
        "doc.md",
        &TextExpectation::Content {
            content_id: content_id(b"before\n"),
        },
        b"after\n",
        Inject::BeforeRename,
    );
    assert!(
        matches!(settlement, WriteSettlement::Refused(TextFailure::Io(_))),
        "{settlement:?}"
    );
    assert_eq!(std::fs::read(root.join("doc.md")).unwrap(), b"before\n");
    assert_eq!(listing(root), vec!["doc.md".to_string()]);
}

#[test]
fn remote_text_edit_failed_reread_after_rename_still_reports_the_write() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(root, "doc.md", b"before\n");

    let settlement = write_text_inner(
        root,
        "doc.md",
        &TextExpectation::Content {
            content_id: content_id(b"before\n"),
        },
        b"after\n",
        Inject::AfterRename,
    );
    match settlement {
        WriteSettlement::WrittenUnverified { before, bytes, .. } => {
            assert_eq!(before, Some(content_id(b"before\n")));
            assert_eq!(bytes, 6);
        }
        other => panic!("a landed rename is never `Refused`: {other:?}"),
    }
    assert_eq!(std::fs::read(root.join("doc.md")).unwrap(), b"after\n");
    assert_eq!(listing(root), vec!["doc.md".to_string()]);
}

#[test]
fn remote_text_edit_stale_absent_and_unchanged_settle_without_writing() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(root, "a.txt", b"one\n");

    assert_eq!(
        write_text(root, "a.txt", &TextExpectation::Absent {}, b"x"),
        WriteSettlement::Stale {
            current: Some(content_id(b"one\n"))
        }
    );
    assert_eq!(
        write_text(
            root,
            "missing.txt",
            &TextExpectation::Content {
                content_id: content_id(b"one\n")
            },
            b"x"
        ),
        WriteSettlement::Stale { current: None }
    );
    assert_eq!(
        write_text(
            root,
            "a.txt",
            &TextExpectation::Content {
                content_id: content_id(b"other\n")
            },
            b"x"
        ),
        WriteSettlement::Stale {
            current: Some(content_id(b"one\n"))
        }
    );
    assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), b"one\n");

    use std::os::unix::fs::MetadataExt;
    let before = std::fs::metadata(root.join("a.txt")).unwrap();
    assert!(matches!(
        write_text(
            root,
            "a.txt",
            &TextExpectation::Content {
                content_id: content_id(b"one\n")
            },
            b"one\n"
        ),
        WriteSettlement::Unchanged { .. }
    ));
    let after = std::fs::metadata(root.join("a.txt")).unwrap();
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.mtime_nsec(), after.mtime_nsec());

    assert!(matches!(
        write_text(root, "created.txt", &TextExpectation::Absent {}, b"fresh\n"),
        WriteSettlement::Written { before: None, .. }
    ));
    assert_eq!(
        listing(root),
        vec!["a.txt".to_string(), "created.txt".to_string()]
    );
}

fn facts(owner: Option<(&str, bool)>, coverage: WriterCoverage) -> EligibilityFacts {
    EligibilityFacts {
        owner: owner.map(|(status, queued)| (status.to_string(), queued)),
        coverage,
        root_guard_held: false,
        base_merge_in_progress: Some(false),
        is_resolving: false,
    }
}

#[test]
fn remote_text_edit_eligibility_is_evaluated_in_published_order() {
    let reason =
        |facts: &EligibilityFacts| classify_text_eligibility(facts).err().map(|e| e.reason);
    let unknown = || WriterCoverage::Unknown {
        detail: "coverage unknown".to_string(),
    };

    assert_eq!(reason(&facts(None, WriterCoverage::Clear)), None);
    assert_eq!(
        reason(&facts(Some(("not queued", false)), WriterCoverage::Clear)),
        None
    );
    assert_eq!(
        reason(&facts(Some(("error", false)), WriterCoverage::Clear)),
        None
    );

    // Active beats everything after it.
    let mut all = facts(Some(("applying", true)), unknown());
    all.root_guard_held = true;
    assert_eq!(reason(&all), Some(TextEditBlockedReason::ChangeActive));

    for (status, queued) in [("queued", false), ("blocked", false), ("not queued", true)] {
        assert_eq!(
            reason(&facts(Some((status, queued)), unknown())),
            Some(TextEditBlockedReason::ChangeAdmitted),
            "{status}/{queued}"
        );
    }

    // Busy and Unknown both refuse, with the coverage detail carried along.
    let busy = classify_text_eligibility(&facts(
        None,
        WriterCoverage::Busy {
            detail: "a writer is bound here".to_string(),
        },
    ))
    .unwrap_err();
    assert_eq!(busy.reason, TextEditBlockedReason::ManagedProcessLive);
    assert_eq!(busy.detail.as_deref(), Some("a writer is bound here"));
    let mut unknown_and_busy_root = facts(None, unknown());
    unknown_and_busy_root.is_resolving = true;
    assert_eq!(
        reason(&unknown_and_busy_root),
        Some(TextEditBlockedReason::ManagedProcessLive)
    );

    for mutate in [
        (|f: &mut EligibilityFacts| f.root_guard_held = true) as fn(&mut EligibilityFacts),
        |f| f.base_merge_in_progress = Some(true),
        |f| f.base_merge_in_progress = None,
        |f| f.is_resolving = true,
    ] {
        let mut f = facts(None, WriterCoverage::Clear);
        mutate(&mut f);
        assert_eq!(reason(&f), Some(TextEditBlockedReason::RootBusy));
    }
}

#[test]
fn remote_text_edit_capability_publishes_every_reason_and_bound() {
    let capability = capability(true);
    assert!(capability.available);
    assert_eq!(capability.max_bytes, 1_048_576);
    assert_eq!(capability.max_path_bytes, 1024);
    assert_eq!(capability.content_id_algorithm, "sha256");
    assert_eq!(
        capability.reasons,
        [
            "git_metadata",
            "conflux_owned_path",
            "unsafe_path",
            "nested_repository",
            "not_regular_file",
            "executable",
            "too_large",
            "not_text",
            "root_missing",
            "change_active",
            "change_admitted",
            "managed_process_live",
            "root_busy"
        ]
    );
    assert_eq!(
        capability.commands,
        ["write_change_text", "write_worktree_text"]
    );
}
