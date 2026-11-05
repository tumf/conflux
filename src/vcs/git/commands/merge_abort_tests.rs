//! Abort-on-conflict merge: the restoration verdict is derived from repository
//! evidence, and a failing or unreadable verification is reported, not assumed.
//!
//! The real-Git cases inject failures through a shim program that passes every
//! other invocation through to the real `git` resolved before shimming. The shim
//! is handed to the merge as its Git program rather than placed on the
//! process-wide `PATH`, so concurrently running tests never see it.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use tempfile::TempDir;

use super::*;

const PREMERGE: &str = "1111111111111111111111111111111111111111";

fn report(restored: Restoration) -> AbortReport {
    AbortReport {
        premerge_head: PREMERGE.to_string(),
        exit_ok: restored == Restoration::Confirmed,
        head_after: Some(PREMERGE.to_string()),
        merge_head_present: Some(restored != Restoration::Confirmed),
        clean_after: Some(true),
        restored,
        detail: String::new(),
    }
}

// ── Unit: verdict and wording ──────────────────────────────────────────────

#[test]
fn remote_merge_abort_restoration_requires_every_verification() {
    let ok = classify_restoration(PREMERGE, true, Some(PREMERGE), Some(false), Some(true));
    assert_eq!(ok, Restoration::Confirmed);

    // Each single failing fact refuses confirmation.
    for (exit_ok, head, merge_head, clean) in [
        (false, PREMERGE, false, true),
        (
            true,
            "2222222222222222222222222222222222222222",
            false,
            true,
        ),
        (true, PREMERGE, true, true),
        (true, PREMERGE, false, false),
    ] {
        assert_eq!(
            classify_restoration(PREMERGE, exit_ok, Some(head), Some(merge_head), Some(clean)),
            Restoration::Failed,
            "exit_ok={exit_ok} head={head} merge_head={merge_head} clean={clean}"
        );
    }

    // An unreadable verification is unknown, even when the abort exited zero
    // and even when another fact already failed.
    assert_eq!(
        classify_restoration(PREMERGE, true, None, Some(false), Some(true)),
        Restoration::Unknown
    );
    assert_eq!(
        classify_restoration(PREMERGE, false, Some(PREMERGE), None, Some(true)),
        Restoration::Unknown
    );
    assert_eq!(
        classify_restoration(PREMERGE, true, Some(PREMERGE), Some(false), None),
        Restoration::Unknown
    );
}

#[test]
fn remote_merge_abort_message_claims_restoration_only_when_confirmed() {
    let files = vec!["src/lib.rs".to_string()];

    let confirmed = abort_message(&files, &report(Restoration::Confirmed));
    assert!(confirmed
        .contains("Merge aborted; base restored to 1111111111111111111111111111111111111111"));
    assert!(confirmed.contains("Files: src/lib.rs"));

    for restored in [Restoration::Failed, Restoration::Unknown] {
        let text = abort_message(&files, &report(restored));
        assert!(
            !text.contains("Merge aborted"),
            "{restored:?} must not claim the merge was aborted: {text}"
        );
        assert!(text.contains("manual recovery required"), "{text}");
        assert!(text.contains("Files: src/lib.rs"), "{text}");
    }
    assert!(abort_message(&files, &report(Restoration::Failed)).contains("abort FAILED"));
    assert!(abort_message(&files, &report(Restoration::Unknown)).contains("UNCONFIRMED"));
}

#[test]
fn remote_merge_abort_detail_is_bounded_and_sanitized() {
    let long = format!("{}\u{1b}[31mfatal: é\u{7}", "x".repeat(4000));
    let tail = sanitized_tail(&long, MAX_ABORT_DETAIL_BYTES);
    assert!(tail.len() <= MAX_ABORT_DETAIL_BYTES);
    assert!(tail.ends_with("fatal: é"));
    assert!(!tail.chars().any(|c| c.is_control() && c != '\n'));
}

// ── Integration: real Git ──────────────────────────────────────────────────

/// Run one fixture Git command synchronously, isolated from user config.
fn git_sync(cwd: &Path, args: &[&str]) -> String {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A repository on `main` with a `feature` branch editing the same line as a
/// later `main` commit (conflicting) and a `clean` branch adding a new file.
struct Fixture {
    dir: TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git_sync(&root, &["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.email", "test@example.com"),
            ("user.name", "Test User"),
            ("commit.gpgsign", "false"),
        ] {
            git_sync(&root, &["config", key, value]);
        }
        std::fs::write(root.join("shared.txt"), "base\n").unwrap();
        git_sync(&root, &["add", "-A"]);
        git_sync(&root, &["commit", "-q", "-m", "base"]);

        git_sync(&root, &["checkout", "-q", "-b", "feature"]);
        std::fs::write(root.join("shared.txt"), "feature\n").unwrap();
        git_sync(&root, &["commit", "-q", "-am", "feature edit"]);

        git_sync(&root, &["checkout", "-q", "main"]);
        git_sync(&root, &["checkout", "-q", "-b", "clean"]);
        std::fs::write(root.join("added.txt"), "added\n").unwrap();
        git_sync(&root, &["add", "-A"]);
        git_sync(&root, &["commit", "-q", "-m", "clean add"]);

        git_sync(&root, &["checkout", "-q", "main"]);
        std::fs::write(root.join("shared.txt"), "main\n").unwrap();
        git_sync(&root, &["commit", "-q", "-am", "main edit"]);
        Self { dir, root }
    }

    fn head(&self) -> String {
        git_sync(&self.root, &["rev-parse", "HEAD"])
    }

    fn merge_head_present(&self) -> bool {
        self.root.join(".git").join("MERGE_HEAD").exists()
    }

    /// Write an executable shim outside the repository whose `body` runs
    /// before the pass-through to the real `git`.
    fn shim(&self, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let real = real_git();
        let path = self.dir.path().join(name);
        let script = format!(
            "#!/bin/sh\nREAL='{}'\nMARKER='{}'\n{}\nexec \"$REAL\" \"$@\"\n",
            real.display(),
            self.dir.path().join(format!("{name}.aborted")).display(),
            body
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}

/// The real `git`, resolved from `PATH` before any shim exists.
fn real_git() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").expect("PATH is set"))
        .map(|dir| dir.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("git is on PATH")
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_clean_merge_is_merged() {
    let fixture = Fixture::new();
    let before = fixture.head();

    let outcome = merge_branch_aborting_on_conflict(&fixture.root, "clean")
        .await
        .expect("clean merge");
    assert_eq!(outcome, AbortingMergeOutcome::Merged);

    let parents = git_sync(&fixture.root, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(
        parents.split_whitespace().count(),
        3,
        "--no-ff produces a two-parent merge commit: {parents}"
    );
    assert!(parents.contains(&before));
    assert!(!fixture.merge_head_present());
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_real_conflict_aborts_and_verifies_restoration() {
    let fixture = Fixture::new();
    let before = fixture.head();

    let AbortingMergeOutcome::Conflict { files, abort } =
        merge_branch_aborting_on_conflict(&fixture.root, "feature")
            .await
            .expect("conflict is a value")
    else {
        panic!("the feature branch must conflict");
    };
    assert_eq!(files, vec!["shared.txt".to_string()]);
    assert_eq!(abort.premerge_head, before);
    assert!(abort.exit_ok);
    assert_eq!(abort.head_after.as_deref(), Some(before.as_str()));
    assert_eq!(abort.merge_head_present, Some(false));
    assert_eq!(abort.clean_after, Some(true));
    assert_eq!(abort.restored, Restoration::Confirmed);
    assert!(!fixture.merge_head_present());
    assert_eq!(fixture.head(), before);

    // The pass/fail wrapper names the verified outcome.
    let error = merge_branch(&fixture.root, "feature")
        .await
        .expect_err("conflict is an error for the wrapper")
        .to_string();
    assert!(error.contains("Merge aborted; base restored to"), "{error}");
    assert!(error.contains("shared.txt"), "{error}");

    // A following merge of a clean branch is admissible.
    assert_eq!(
        merge_branch_aborting_on_conflict(&fixture.root, "clean")
            .await
            .expect("next merge"),
        AbortingMergeOutcome::Merged
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_failed_abort_is_reported_not_assumed() {
    let fixture = Fixture::new();
    let before = fixture.head();
    let shim = fixture.shim(
        "git-fail-abort",
        "if [ \"$1\" = merge ] && [ \"$2\" = --abort ]; then echo 'fatal: injected abort failure' >&2; exit 128; fi",
    );

    let AbortingMergeOutcome::Conflict { files, abort } =
        aborting_merge_with(shim.as_os_str(), &fixture.root, "feature")
            .await
            .expect("conflict is a value")
    else {
        panic!("the feature branch must conflict");
    };
    assert_eq!(files, vec!["shared.txt".to_string()]);
    assert!(!abort.exit_ok);
    assert_eq!(abort.merge_head_present, Some(true));
    assert_eq!(abort.head_after.as_deref(), Some(before.as_str()));
    assert_eq!(abort.restored, Restoration::Failed);
    assert!(
        abort.detail.contains("injected abort failure"),
        "{}",
        abort.detail
    );
    assert!(
        fixture.merge_head_present(),
        "nothing compensated the failed abort"
    );

    let text = abort_message(&files, &abort);
    assert!(text.contains("abort FAILED"), "{text}");
    assert!(!text.contains("Merge aborted"), "{text}");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_unreadable_verification_is_unknown() {
    let fixture = Fixture::new();
    // The abort runs for real; afterwards `rev-parse --verify HEAD` fails.
    let shim = fixture.shim(
        "git-unreadable-head",
        "if [ \"$1\" = merge ] && [ \"$2\" = --abort ]; then \"$REAL\" \"$@\"; rc=$?; : > \"$MARKER\"; exit $rc; fi\n\
         if [ -e \"$MARKER\" ] && [ \"$1\" = rev-parse ] && [ \"$2\" = --verify ] && [ \"$3\" = HEAD ]; then exit 128; fi",
    );

    let AbortingMergeOutcome::Conflict { files, abort } =
        aborting_merge_with(shim.as_os_str(), &fixture.root, "feature")
            .await
            .expect("conflict is a value")
    else {
        panic!("the feature branch must conflict");
    };
    assert!(abort.exit_ok);
    assert_eq!(abort.head_after, None);
    assert_eq!(abort.restored, Restoration::Unknown);
    let text = abort_message(&files, &abort);
    assert!(text.contains("UNCONFIRMED"), "{text}");
    assert!(!text.contains("Merge aborted"), "{text}");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_non_conflict_failure_is_an_error() {
    let fixture = Fixture::new();
    let before = fixture.head();

    // A ref Git cannot merge fails before any merge state exists: no conflict,
    // so no abort is attempted and the plain failure is reported.
    let error = merge_branch_aborting_on_conflict(&fixture.root, "does-not-exist")
        .await
        .expect_err("an unmergeable ref is an error, not a conflict")
        .to_string();
    assert!(error.contains("git merge does-not-exist failed"), "{error}");
    assert!(!fixture.merge_head_present());
    assert_eq!(fixture.head(), before);
}
