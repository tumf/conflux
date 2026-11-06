//! Content identity oracle regressions.
//!
//! The parsing and hashing cases are pure. Everything that observes a worktree
//! runs real Git against a disposable repository, so it is heavy.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use super::*;

// ── Fixture ─────────────────────────────────────────────────────────────────

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// A committed repository: `README` holds `A`, `target/` and `*.log` are ignored.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.email", "test@example.com"),
        ("user.name", "Test User"),
        ("commit.gpgsign", "false"),
        ("protocol.file.allow", "always"),
    ] {
        git(root, &["config", key, value]);
    }
    write(root, "README", "A");
    write(root, ".gitignore", "target/\n*.log\n");
    write(root, "docs/guide.md", "guide\n");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "base"]);
    dir
}

fn write(root: &Path, rel: &str, contents: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn observe(root: &Path) -> ObservedContent {
    observe_with(root, &ConsentLimits::PRODUCTION)
}

fn observe_with(root: &Path, limits: &ConsentLimits) -> ObservedContent {
    match observe_discardable_content(root, limits) {
        ContentObservation::Observed(content) => content,
        other => panic!("expected an observed identity, got {other:?}"),
    }
}

fn records(root: &Path) -> (Vec<String>, Vec<String>) {
    observe_records(root, &ConsentLimits::PRODUCTION).expect("records bind")
}

fn dirty(root: &Path) -> bool {
    !git(
        root,
        &[
            "status",
            "--porcelain",
            "--untracked-files=normal",
            "--ignored=no",
        ],
    )
    .trim()
    .is_empty()
}

/// Rewrite a file to new bytes and put its mtime and mode back exactly.
fn rewrite_preserving_metadata(path: &Path, contents: &[u8]) {
    use std::os::unix::fs::MetadataExt;
    let before = std::fs::symlink_metadata(path).unwrap();
    let modified = before.modified().unwrap();
    let permissions = before.permissions();
    std::fs::write(path, contents).unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    drop(file);
    std::fs::set_permissions(path, permissions).unwrap();
    let after = std::fs::symlink_metadata(path).unwrap();
    assert_eq!(before.len(), after.len(), "size must be unchanged");
    assert_eq!(before.mode(), after.mode(), "mode must be unchanged");
    assert_eq!(
        (before.mtime(), before.mtime_nsec()),
        (after.mtime(), after.mtime_nsec()),
        "mtime must be restored"
    );
}

/// Every name, type, and byte under `root`, `.git` included, to prove the
/// oracle writes nothing.
fn tree_digest(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                out.push((path.clone(), b"dir".to_vec()));
                visit(&path, out);
            } else if meta.is_file() {
                out.push((path.clone(), std::fs::read(&path).unwrap()));
            } else {
                out.push((path.clone(), b"other".to_vec()));
            }
        }
    }
    let mut out = Vec::new();
    visit(root, &mut out);
    out
}

// ── Pure ────────────────────────────────────────────────────────────────────

#[test]
fn remote_confirmed_delete_production_limits_are_the_documented_budget() {
    assert_eq!(ConsentLimits::PRODUCTION.max_records, 500_000);
    assert_eq!(
        ConsentLimits::PRODUCTION.max_hashed_bytes,
        256 * 1024 * 1024
    );
    assert_eq!(ConsentLimits::default(), ConsentLimits::PRODUCTION);
}

#[test]
fn remote_confirmed_delete_empty_identity_is_a_well_formed_content_id() {
    let empty = empty_content_id();
    assert!(is_content_id(&empty), "{empty}");
    assert_eq!(
        empty,
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert!(!is_content_id("sha256:ABC"));
    assert!(!is_content_id(&empty.replace("sha256:", "md5:")));
}

#[test]
fn remote_confirmed_delete_status_parser_is_closed() {
    let status = b"1 .M N... 100644 100644 100644 aaaa bbbb README\x002 R. N... 100644 100644 100644 aaaa aaaa R100 new name\x00old name\x00? dir/\x00! target/\x00";
    let parsed = parse_status(status).expect("parses");
    assert_eq!(parsed.len(), 4);
    assert_eq!(parsed[0].path, b"README");
    assert_eq!(parsed[1].path, b"new name");
    assert_eq!(parsed[1].original.as_deref(), Some(&b"old name"[..]));
    assert_eq!(parsed[2].path, b"dir/");
    assert_eq!(parsed[3].kind, b'!');
    let submodule = parse_status(b"1 .M SC.. 160000 160000 160000 aaaa aaaa sub\x00").unwrap();
    assert!(submodule[0].submodule);
    assert!(
        parse_status(b"X what\x00").is_none(),
        "unknown kinds fail closed"
    );
    assert!(
        parse_status(b"1 .M N...\x00").is_none(),
        "truncated records fail closed"
    );
}

// ── Real Git ────────────────────────────────────────────────────────────────

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_confirmed_delete_content_identity_is_byte_sensitive() {
    let dir = repo();
    let root = dir.path();

    let clean = observe(root);
    assert_eq!(clean.discardable_content_id, empty_content_id());
    assert_eq!(clean.ignored_content_id, empty_content_id());
    assert!(!clean.has_ignored_content);
    assert_eq!(clean.entries, 0);

    // The oracle writes nothing: not in the worktree, not in `.git`.
    let digest = tree_digest(root);
    write(root, "README", "A");
    let _ = observe(root);
    assert_eq!(tree_digest(root), digest, "the oracle must not write");

    write(root, "README", "B");
    let modified_b = observe(root);
    assert_eq!(observe(root), modified_b, "same bytes twice are equal");
    write(root, "README", "C");
    let modified_c = observe(root);
    assert_ne!(
        modified_b.discardable_content_id,
        clean.discardable_content_id
    );
    assert_ne!(
        modified_b.discardable_content_id, modified_c.discardable_content_id,
        "README B and C under the same HEAD and dirty flag must differ"
    );

    git(root, &["add", "README"]);
    let staged = observe(root);
    assert_ne!(
        staged.discardable_content_id,
        modified_c.discardable_content_id
    );
    git(root, &["reset", "-q", "--hard"]);

    write(root, "new.txt", "untracked\n");
    let untracked = observe(root);
    assert_ne!(
        untracked.discardable_content_id,
        clean.discardable_content_id
    );
    std::fs::remove_file(root.join("new.txt")).unwrap();

    std::os::unix::fs::symlink("README", root.join("link")).unwrap();
    let symlinked = observe(root);
    assert_ne!(
        symlinked.discardable_content_id,
        clean.discardable_content_id
    );
    std::fs::remove_file(root.join("link")).unwrap();

    std::fs::remove_file(root.join("docs/guide.md")).unwrap();
    let deleted = observe(root);
    assert_ne!(deleted.discardable_content_id, clean.discardable_content_id);
    git(root, &["reset", "-q", "--hard"]);

    git(root, &["mv", "docs/guide.md", "docs/renamed.md"]);
    let renamed = observe(root);
    assert_ne!(renamed.discardable_content_id, clean.discardable_content_id);
    git(root, &["reset", "-q", "--hard"]);
    assert_eq!(
        observe(root),
        clean,
        "reverting restores the clean identity"
    );

    let not_a_repo = TempDir::new().unwrap();
    assert_eq!(
        observe_discardable_content(not_a_repo.path(), &ConsentLimits::PRODUCTION),
        ContentObservation::Unobservable,
        "a failed status is unobservable, never an empty identity"
    );
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_confirmed_delete_content_identity_binds_ignored_paths() {
    let dir = repo();
    let root = dir.path();
    let clean = observe(root);

    write(root, "build.log", "one\n");
    assert!(
        !dirty(root),
        "an ignored file leaves the dirty observation clean"
    );
    let added = observe(root);
    assert!(added.has_ignored_content);
    assert_eq!(added.ignored_entries, 1);
    assert_ne!(added.ignored_content_id, clean.ignored_content_id);
    assert_eq!(added.discardable_content_id, clean.discardable_content_id);

    write(root, "build.log", "two\n");
    let modified = observe(root);
    assert_ne!(modified.ignored_content_id, added.ignored_content_id);
    assert_eq!(
        modified.discardable_content_id,
        clean.discardable_content_id
    );

    std::fs::remove_file(root.join("build.log")).unwrap();
    let removed = observe(root);
    assert!(!removed.has_ignored_content);
    assert_eq!(removed.ignored_content_id, clean.ignored_content_id);

    // Git reports this as both `? fresh/` and `! fresh/x.log`: the ignored
    // file is in the ignored identity only, read once.
    write(root, "fresh/keep.txt", "keep\n");
    write(root, "fresh/x.log", "log\n");
    let (discardable, ignored) = records(root);
    assert_eq!(ignored.len(), 1, "{ignored:?}");
    assert!(ignored[0].starts_with("fresh/x.log|"), "{ignored:?}");
    assert!(
        !discardable.iter().any(|r| r.starts_with("fresh/x.log|")),
        "an ignored path must not also be discardable: {discardable:?}"
    );
    assert!(discardable.iter().any(|r| r.starts_with("fresh/keep.txt|")));
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_confirmed_delete_same_size_bytes_mutated_with_mtime_restored_changes_identity() {
    let dir = repo();
    let root = dir.path();
    write(root, "target/a.o", "AAAA");
    let before = observe(root);
    assert!(before.has_ignored_content);
    assert!(!dirty(root));

    rewrite_preserving_metadata(&root.join("target/a.o"), b"AAAB");
    let after = observe(root);
    assert_ne!(
        after.ignored_content_id, before.ignored_content_id,
        "a metadata-only identity cannot pass this: the bytes must be bound"
    );
    assert_eq!(after.discardable_content_id, before.discardable_content_id);
    assert!(after.has_ignored_content);
    assert!(!dirty(root));

    // The same rewrite of tracked content changes the discardable identity.
    write(root, "README", "B");
    let b = observe(root);
    rewrite_preserving_metadata(&root.join("README"), b"C");
    let c = observe(root);
    assert_ne!(b.discardable_content_id, c.discardable_content_id);
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_confirmed_delete_ignored_directory_walk_binds_descendant_drift() {
    let dir = repo();
    let root = dir.path();
    write(root, "target/deep/deeper/b.o", "bbbb");
    write(root, "target/top.o", "top");

    let status = git(
        root,
        &[
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=normal",
            "--ignored=traditional",
        ],
    );
    assert_eq!(
        status, "! target/\0",
        "target/ must be one collapsed record"
    );
    assert!(!dirty(root));

    let mut previous = observe(root);
    let discardable = previous.discardable_content_id.clone();
    let mut step = |label: &str, mutate: &dyn Fn()| {
        mutate();
        let next = match observe_discardable_content(root, &ConsentLimits::PRODUCTION) {
            ContentObservation::Observed(next) => next,
            other => panic!("{label}: a plain ignored directory is walked, got {other:?}"),
        };
        assert_ne!(
            next.ignored_content_id, previous.ignored_content_id,
            "{label} must change the ignored identity"
        );
        assert_eq!(next.discardable_content_id, discardable, "{label}");
        assert!(!dirty(root), "{label}");
        previous = next;
    };

    step("same-size rewrite three levels deep", &|| {
        rewrite_preserving_metadata(&root.join("target/deep/deeper/b.o"), b"bbbc")
    });
    step("new file three levels deep", &|| {
        write(root, "target/deep/deeper/c.o", "c")
    });
    step("removed file", &|| {
        std::fs::remove_file(root.join("target/deep/deeper/c.o")).unwrap()
    });
    step("new empty subdirectory", &|| {
        std::fs::create_dir(root.join("target/empty")).unwrap()
    });
    step("removed empty subdirectory", &|| {
        std::fs::remove_dir(root.join("target/empty")).unwrap()
    });
    step("file replaced by a symlink", &|| {
        std::fs::remove_file(root.join("target/top.o")).unwrap();
        std::os::unix::fs::symlink("deep", root.join("target/top.o")).unwrap();
    });
    step("symlink target string changed", &|| {
        std::fs::remove_file(root.join("target/top.o")).unwrap();
        std::os::unix::fs::symlink("elsewhere", root.join("target/top.o")).unwrap();
    });

    // A symlink to a directory outside is bound by its target string only:
    // what lies behind it is neither read nor bound, and the walk terminates.
    let outside = TempDir::new().unwrap();
    write(outside.path(), "behind.txt", "one");
    std::os::unix::fs::symlink(outside.path(), root.join("target/out")).unwrap();
    let linked = observe(root);
    write(outside.path(), "behind.txt", "two");
    assert_eq!(
        observe(root),
        linked,
        "content behind a symlink is not bound"
    );

    // An actual nested repository, as a real repository or as a gitfile.
    git(root, &["init", "-q", "target/nested"]);
    assert_eq!(
        observe_discardable_content(root, &ConsentLimits::PRODUCTION),
        ContentObservation::NestedRepository
    );
    std::fs::remove_dir_all(root.join("target/nested")).unwrap();
    write(root, "target/wt/.git", "gitdir: /nowhere\n");
    assert_eq!(
        observe_discardable_content(root, &ConsentLimits::PRODUCTION),
        ContentObservation::NestedRepository
    );
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_confirmed_delete_bound_exceeded_fails_closed() {
    let dir = repo();
    let root = dir.path();
    for index in 0..6 {
        write(root, &format!("target/f{index}.o"), "0123456789");
    }
    assert!(!dirty(root));
    let fits = ConsentLimits {
        max_records: 100,
        max_hashed_bytes: 1000,
        ..ConsentLimits::PRODUCTION
    };
    assert!(observe_with(root, &fits).has_ignored_content);

    // Records: one `! target/` record plus six walked entries exceeds five.
    let few_records = ConsentLimits {
        max_records: 5,
        max_hashed_bytes: 1000,
        ..ConsentLimits::PRODUCTION
    };
    assert_eq!(
        observe_discardable_content(root, &few_records),
        ContentObservation::Unbounded,
        "ignored entries alone exceed the record cap"
    );

    // Bytes: sixty ignored bytes exceed fifty.
    let few_bytes = ConsentLimits {
        max_records: 100,
        max_hashed_bytes: 50,
        ..ConsentLimits::PRODUCTION
    };
    assert_eq!(
        observe_discardable_content(root, &few_bytes),
        ContentObservation::Unbounded,
        "ignored bytes alone exceed the byte cap"
    );

    // One file larger than the whole remaining budget: no partial hash.
    write(root, "target/big.o", &"x".repeat(200));
    let one_big = ConsentLimits {
        max_records: 100,
        max_hashed_bytes: 150,
        ..ConsentLimits::PRODUCTION
    };
    assert_eq!(
        observe_discardable_content(root, &one_big),
        ContentObservation::Unbounded
    );
    std::fs::remove_file(root.join("target/big.o")).unwrap();

    // Combined: tracked bytes and ignored bytes each fit alone, not together.
    std::fs::remove_dir_all(root.join("target")).unwrap();
    write(root, "target/only.o", &"i".repeat(40));
    let combined = ConsentLimits {
        max_records: 100,
        max_hashed_bytes: 60,
        ..ConsentLimits::PRODUCTION
    };
    assert!(observe_with(root, &combined).has_ignored_content);
    write(root, "README", &"t".repeat(40));
    assert_eq!(
        observe_discardable_content(root, &combined),
        ContentObservation::Unbounded,
        "tracked plus ignored content share one budget"
    );
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_confirmed_delete_oracle_is_nonblocking_and_bounded() {
    fn mkfifo(path: &Path) {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: `c` is a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o644) }, 0);
    }

    let dir = repo();
    let root = dir.path();

    // A tracked path replaced by a FIFO with no writer: recorded by type,
    // never opened. Completing at all is the non-blocking assertion. Git
    // itself reports nothing for a FIFO at a tracked path, so the status is
    // captured while `README` is an ordinary modification and the FIFO is
    // swapped in before the oracle reads it — the race it must survive.
    write(root, "README", "B");
    let status = Command::new("git")
        .args(read_only_status_argv(CONSENT_STATUS_ARGS))
        .current_dir(root)
        .output()
        .unwrap()
        .stdout;
    std::fs::remove_file(root.join("README")).unwrap();
    mkfifo(&root.join("README"));
    let (discardable, _) =
        observe_records_from_status(root, &status, &ConsentLimits::PRODUCTION).unwrap();
    assert!(
        discardable
            .iter()
            .any(|r| r.starts_with("README|") && r.ends_with("|o:fifo")),
        "{discardable:?}"
    );
    std::fs::remove_file(root.join("README")).unwrap();
    git(root, &["checkout", "--", "README"]);

    // A FIFO inside a walked ignored directory. A directory holding only
    // special files produces no status record at all, so `target/` carries one
    // regular file to be reported, and walked, as `! target/`.
    write(root, "target/keep.o", "k");
    std::fs::create_dir_all(root.join("target/deep")).unwrap();
    mkfifo(&root.join("target/deep/fifo"));
    let (_, ignored) = records(root);
    assert!(
        ignored.contains(&"target/deep/fifo|w|i:o:fifo".to_string()),
        "{ignored:?}"
    );
    std::fs::remove_dir_all(root.join("target")).unwrap();

    // A nested untracked repository.
    git(root, &["init", "-q", "vendor"]);
    assert_eq!(
        observe_discardable_content(root, &ConsentLimits::PRODUCTION),
        ContentObservation::NestedRepository
    );
    std::fs::remove_dir_all(root.join("vendor")).unwrap();

    // A modified submodule.
    let upstream = repo();
    git(
        root,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            upstream.path().to_str().unwrap(),
            "sub",
        ],
    );
    git(root, &["commit", "-q", "-m", "submodule"]);
    write(root, "sub/README", "changed");
    assert_eq!(
        observe_discardable_content(root, &ConsentLimits::PRODUCTION),
        ContentObservation::NestedRepository
    );
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_confirmed_delete_swapped_parent_component_is_not_followed() {
    let dir = repo();
    let root = dir.path();
    write(root, "docs/guide.md", "edited\n");
    let status = Command::new("git")
        .args(read_only_status_argv(CONSENT_STATUS_ARGS))
        .current_dir(root)
        .output()
        .unwrap()
        .stdout;

    // After the status call, `docs` becomes a symlink to a directory holding
    // a file of the same name.
    let elsewhere = TempDir::new().unwrap();
    write(elsewhere.path(), "guide.md", "behind the link\n");
    std::fs::rename(root.join("docs"), root.join("docs-real")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), root.join("docs")).unwrap();

    let (discardable, _) =
        observe_records_from_status(root, &status, &ConsentLimits::PRODUCTION).unwrap();
    assert!(
        discardable
            .iter()
            .any(|r| r.starts_with("docs/guide.md|") && r.ends_with("|o:parent")),
        "{discardable:?}"
    );
    let first = observe_from_status(root, &status, &ConsentLimits::PRODUCTION);
    write(elsewhere.path(), "guide.md", "changed behind the link\n");
    assert_eq!(
        observe_from_status(root, &status, &ConsentLimits::PRODUCTION),
        first,
        "content behind a swapped component is never read"
    );
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_confirmed_delete_walk_depth_is_bounded() {
    let dir = repo();
    let root = dir.path();
    let mut deep = root.join("target");
    for _ in 0..(unix::MAX_WALK_DEPTH + 2) {
        deep = deep.join("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("leaf.o"), "x").unwrap();
    assert_eq!(
        observe_discardable_content(root, &ConsentLimits::PRODUCTION),
        ContentObservation::Unbounded,
        "a tree deeper than the walk bound fails closed"
    );
}

// ── Contract gaps: budgeted listing ─────────────────────────────────────────

/// Open descriptors of this process that point inside `dir`.
///
/// Scoped to one fixture's directory, so descriptors other tests hold while
/// running in parallel never count.
#[cfg(target_os = "linux")]
fn fds_under(dir: &Path) -> usize {
    let dir = std::fs::canonicalize(dir).unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
        .filter(|target| target.starts_with(&dir))
        .count()
}

#[cfg(not(target_os = "linux"))]
fn fds_under(_dir: &Path) -> usize {
    0
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_contract_gap_delete_single_directory_is_bounded_before_materialization() {
    let dir = repo();
    let root = dir.path();
    for index in 0..20 {
        write(root, &format!("target/f{index:02}.o"), "x");
    }
    let limits = ConsentLimits {
        max_records: 5,
        ..ConsentLimits::PRODUCTION
    };
    let before = fds_under(root);
    let counters = observe_counters(root, &limits);
    assert_eq!(counters.observation, ContentObservation::Unbounded);
    assert!(
        counters.records <= limits.max_records + 1,
        "records charged: {}",
        counters.records
    );
    assert!(
        counters.last_listing_retained <= limits.max_records + 1,
        "names retained: {}",
        counters.last_listing_retained
    );
    assert_eq!(fds_under(root), before, "no directory descriptor leaked");

    // The same tree fits a budget that holds it.
    let fits = ConsentLimits {
        max_records: 100,
        ..ConsentLimits::PRODUCTION
    };
    let counters = observe_counters(root, &fits);
    assert!(matches!(
        counters.observation,
        ContentObservation::Observed(_)
    ));
    assert_eq!(counters.last_listing_retained, 20);
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_contract_gap_delete_listing_name_bytes_are_bounded() {
    let dir = repo();
    let root = dir.path();
    // Twenty names of five bytes each: 100 listing bytes in one directory.
    for index in 0..20 {
        write(root, &format!("target/f{index:02}.o"), "x");
    }
    let limits = ConsentLimits {
        max_listing_name_bytes: 30,
        ..ConsentLimits::PRODUCTION
    };
    let before = fds_under(root);
    let counters = observe_counters(root, &limits);
    assert_eq!(counters.observation, ContentObservation::Unbounded);
    assert!(
        counters.last_listing_retained * 5 <= 30,
        "names retained past the byte budget: {}",
        counters.last_listing_retained
    );
    assert_eq!(fds_under(root), before);

    let fits = ConsentLimits {
        max_listing_name_bytes: 100,
        ..ConsentLimits::PRODUCTION
    };
    assert!(observe_with(root, &fits).has_ignored_content);
}

#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn remote_contract_gap_delete_dir_stream_closes_on_nested_and_unobservable() {
    let dir = repo();
    let root = dir.path();
    write(root, "target/a.o", "x");
    write(root, "target/wt/.git", "gitdir: /nowhere\n");
    let before = fds_under(root);
    assert_eq!(
        observe_discardable_content(root, &ConsentLimits::PRODUCTION),
        ContentObservation::NestedRepository
    );
    assert_eq!(fds_under(root), before, "nested path closes every stream");
    std::fs::remove_dir_all(root.join("target/wt")).unwrap();

    // An unreadable directory below a walked one. Root reads it anyway, so
    // the unobservable half only runs unprivileged.
    use std::os::unix::fs::PermissionsExt;
    let locked = root.join("target/locked");
    std::fs::create_dir_all(&locked).unwrap();
    write(root, "target/locked/inner.o", "x");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    // SAFETY: `geteuid` has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        assert_eq!(
            observe_discardable_content(root, &ConsentLimits::PRODUCTION),
            ContentObservation::Unobservable
        );
        assert_eq!(
            fds_under(root),
            before,
            "unobservable path closes every stream"
        );
    }
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn remote_contract_gap_delete_production_limits_name_the_listing_budget() {
    assert_eq!(
        ConsentLimits::PRODUCTION.max_listing_name_bytes,
        64 * 1024 * 1024
    );
    assert_eq!(ConsentLimits::default(), ConsentLimits::PRODUCTION);
    assert_eq!(ConsentLimits::PRODUCTION.max_records, 500_000);
    assert_eq!(
        ConsentLimits::PRODUCTION.max_hashed_bytes,
        256 * 1024 * 1024
    );
}

#[test]
fn remote_contract_gap_delete_detached_head_is_a_closed_reason() {
    assert_eq!(ConfirmedDeleteReason::ALL.len(), 20);
    assert_eq!(
        ConfirmedDeleteReason::DetachedHead.as_str(),
        "detached_head"
    );
    assert!(!ConfirmedDeleteReason::DetachedHead.is_stale());
    assert_eq!(
        serde_json::to_value(ConfirmedDeleteReason::DetachedHead).unwrap(),
        serde_json::json!("detached_head")
    );
}
