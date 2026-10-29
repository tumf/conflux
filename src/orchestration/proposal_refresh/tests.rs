//! Candidate proposal refresh: the pure eligibility table (unit) and the Git
//! steps against disposable repositories (integration, real `git` and real
//! hooks under a temporary directory).

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use super::*;

// ── Fixture ────────────────────────────────────────────────────────────────

/// The change every fixture refreshes.
pub(crate) const CHANGE: &str = "alpha";

/// Run one fixture Git command synchronously, isolated from user config.
pub(crate) fn git_sync(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
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

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("fixture path has a parent")).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// A base repository on `develop`, a candidate worktree holding an applied
/// implementation commit, and a corrected proposal committed on `develop`.
pub(crate) struct Fixture {
    _dir: TempDir,
    /// Project repository root (main worktree, on `develop`).
    pub root: PathBuf,
    /// Candidate managed worktree for [`CHANGE`].
    pub worktree: PathBuf,
}

pub(crate) const STALE_PROPOSAL: &str = "# alpha\n\nverification: stale\n";
pub(crate) const CORRECTED_PROPOSAL: &str = "# alpha\n\nverification: corrected\n";

impl Fixture {
    pub(crate) fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git_sync(&root, &["init", "-q", "-b", "develop"]);
        // Repository-local configuration, so the commit step inside the
        // production code (which inherits the environment) reads exactly this
        // and never a developer's global hooks or signing setup.
        git_sync(&root, &["config", "user.name", "cflx"]);
        git_sync(&root, &["config", "user.email", "cflx@example.com"]);
        git_sync(&root, &["config", "commit.gpgsign", "false"]);
        let hooks = root.join(".git").join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        git_sync(
            &root,
            &["config", "core.hooksPath", hooks.to_str().unwrap()],
        );

        write(&root, &proposal_path(CHANGE), STALE_PROPOSAL);
        write(
            &root,
            &format!("openspec/changes/{CHANGE}/tasks.md"),
            "- [ ] task\n",
        );
        write(&root, "src/lib.rs", "pub fn base() {}\n");
        git_sync(&root, &["add", "-A"]);
        git_sync(&root, &["commit", "-q", "-m", "base"]);

        let worktree = dir.path().join("worktrees").join(CHANGE);
        git_sync(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                CHANGE,
                worktree.to_str().unwrap(),
            ],
        );
        write(&worktree, "src/impl.rs", "pub fn applied() {}\n");
        write(
            &worktree,
            &format!("openspec/changes/{CHANGE}/tasks.md"),
            "- [x] task\n",
        );
        git_sync(&worktree, &["add", "-A"]);
        git_sync(&worktree, &["commit", "-q", "-m", "Apply: alpha"]);

        write(&root, &proposal_path(CHANGE), CORRECTED_PROPOSAL);
        git_sync(&root, &["commit", "-q", "-am", "correct alpha proposal"]);

        Self {
            _dir: dir,
            root,
            worktree,
        }
    }

    pub(crate) fn rev(&self, cwd: &Path, rev: &str) -> String {
        git_sync(cwd, &["rev-parse", rev])
    }

    pub(crate) fn develop(&self) -> String {
        self.rev(&self.root, "develop")
    }

    pub(crate) fn head(&self) -> String {
        self.rev(&self.worktree, "HEAD")
    }

    pub(crate) fn status(&self) -> String {
        git_sync(
            &self.worktree,
            &crate::vcs::git::commands::status_policy::read_only_status_argv(&[
                "--porcelain=v1",
                "--untracked-files=all",
            ]),
        )
    }

    pub(crate) fn request(&self) -> RefreshRequest {
        RefreshRequest {
            change_id: CHANGE.to_string(),
            source_ref: "develop".to_string(),
            expected_source: self.develop(),
            expected_head: self.head(),
        }
    }

    /// Install an executable `pre-commit` hook in the fixture's hooks directory.
    pub(crate) fn hook(&self, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.join(".git").join("hooks").join("pre-commit");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    pub(crate) fn candidate_text(&self) -> String {
        std::fs::read_to_string(self.worktree.join(proposal_path(CHANGE))).unwrap()
    }
}

// ── Eligibility table (unit) ───────────────────────────────────────────────

#[test]
fn candidate_proposal_refresh_classifier_admits_exactly_the_frozen_rows() {
    use BlockerKind::{Dependency, External, None as NoBlocker};
    use RefreshReason::*;

    let allowed = [
        ("error", NoBlocker),
        ("stopped", NoBlocker),
        ("stalled", NoBlocker),
        ("blocked", External),
        ("not queued", NoBlocker),
    ];
    for (status, blocker) in allowed {
        assert_eq!(
            classify_refresh_proposal(status, blocker, true, true, false),
            RefreshAdmission::Allowed,
            "{status}"
        );
    }

    let refused: [(&str, BlockerKind, bool, bool, bool, RefreshReason); 17] = [
        ("error", NoBlocker, false, true, false, UnknownTarget),
        ("error", NoBlocker, true, false, false, NoCandidateWorktree),
        (
            "not queued",
            NoBlocker,
            true,
            false,
            false,
            NoCandidateWorktree,
        ),
        ("archived", NoBlocker, true, true, false, TerminalTarget),
        ("merged", NoBlocker, true, true, false, TerminalTarget),
        ("pushed", NoBlocker, true, true, false, TerminalTarget),
        ("rejected", NoBlocker, true, true, false, TerminalTarget),
        ("preparing", NoBlocker, true, true, false, TargetActive),
        ("applying", NoBlocker, true, true, false, TargetActive),
        ("accepting", NoBlocker, true, true, false, TargetActive),
        ("resolving", NoBlocker, true, true, false, TargetActive),
        // An eligible status that still owns a live managed process is active.
        ("error", NoBlocker, true, true, true, TargetActive),
        ("queued", NoBlocker, true, true, false, TargetAdmitted),
        ("blocked", Dependency, true, true, false, TargetAdmitted),
        ("merge wait", NoBlocker, true, true, false, MergeWait),
        ("resolve pending", NoBlocker, true, true, false, ResolveWait),
        ("reject pending", NoBlocker, true, true, false, RejectWait),
    ];
    for (status, blocker, tracked, worktree, process, reason) in refused {
        assert_eq!(
            classify_refresh_proposal(status, blocker, tracked, worktree, process),
            RefreshAdmission::Refused(reason),
            "{status} tracked={tracked} worktree={worktree} process={process}"
        );
    }

    // A hold with no structured blocker is work the owner clears by itself.
    assert_eq!(
        classify_refresh_proposal("blocked", NoBlocker, true, true, false),
        RefreshAdmission::Refused(TargetAdmitted)
    );
}

#[test]
fn candidate_proposal_refresh_reason_tokens_are_unique_and_split_by_phase() {
    let all: Vec<RefreshReason> = RefreshReason::LIFECYCLE
        .iter()
        .chain(RefreshReason::COMMAND_TIME.iter())
        .copied()
        .collect();
    let tokens: std::collections::HashSet<&str> = all.iter().map(|r| r.as_str()).collect();
    assert_eq!(tokens.len(), all.len());
    for reason in all {
        assert_eq!(RefreshReason::parse(reason.as_str()), Some(reason));
        assert_eq!(
            reason.is_lifecycle(),
            RefreshReason::LIFECYCLE.contains(&reason)
        );
    }
    assert!(!RefreshReason::HookRejected.is_lifecycle());
    assert!(RefreshReason::TargetActive.is_lifecycle());
}

#[test]
fn candidate_proposal_refresh_full_oid_rejects_short_or_uppercase_ids() {
    assert!(is_full_oid(&"a".repeat(40)));
    assert!(is_full_oid(&"0".repeat(64)));
    assert!(!is_full_oid(&"a".repeat(39)));
    assert!(!is_full_oid(&"A".repeat(40)));
    assert!(!is_full_oid("develop"));
}

// ── Git steps (integration) ────────────────────────────────────────────────

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_preview_reports_facts_and_mutates_nothing() {
    let fx = Fixture::new();
    let head = fx.head();
    let develop = fx.develop();

    let preview = preview(&fx.root, &fx.worktree, CHANGE, "develop").await;

    assert!(preview.eligible, "{preview:?}");
    assert_eq!(preview.refusal, None);
    assert_eq!(preview.source_commit.as_deref(), Some(develop.as_str()));
    assert_eq!(preview.candidate_head.as_deref(), Some(head.as_str()));
    assert_eq!(preview.path, "openspec/changes/alpha/proposal.md");
    assert_eq!(
        preview.source_blob.as_deref(),
        Some(
            fx.rev(&fx.root, &format!("develop:{}", proposal_path(CHANGE)))
                .as_str()
        )
    );
    assert_eq!(
        preview.candidate_blob.as_deref(),
        Some(
            fx.rev(&fx.worktree, &format!("HEAD:{}", proposal_path(CHANGE)))
                .as_str()
        )
    );
    assert!(!preview.no_op);
    let patch = preview
        .patch
        .as_deref()
        .expect("a differing preview carries a patch");
    assert!(patch.contains("-verification: stale"), "{patch}");
    assert!(patch.contains("+verification: corrected"), "{patch}");
    assert_eq!((preview.added_lines, preview.removed_lines), (1, 1));

    // Nothing moved.
    assert_eq!(fx.head(), head);
    assert_eq!(fx.develop(), develop);
    assert_eq!(fx.status(), "");
    assert_eq!(fx.candidate_text(), STALE_PROPOSAL);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_apply_creates_one_scoped_commit_then_settles_unchanged() {
    let fx = Fixture::new();
    let before = fx.head();
    let develop = fx.develop();
    let implementation = std::fs::read(fx.worktree.join("src/impl.rs")).unwrap();
    let tasks = std::fs::read(fx.worktree.join("openspec/changes/alpha/tasks.md")).unwrap();
    fx.hook("echo refresh-hook-ran");

    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;

    assert_eq!(
        report.outcome,
        Some(RefreshOutcome::Refreshed),
        "{report:?}"
    );
    assert_eq!(report.refusal, None);
    assert_eq!(
        report.effects,
        RefreshEffects {
            file_written: true,
            staged: true,
            committed: true,
            clean_after: true,
        }
    );
    let after = fx.head();
    assert_eq!(
        report.candidate_head_before.as_deref(),
        Some(before.as_str())
    );
    assert_eq!(report.candidate_head_after.as_deref(), Some(after.as_str()));
    assert_eq!(report.source_commit.as_deref(), Some(develop.as_str()));
    assert!(report
        .hook_output_tail
        .as_deref()
        .is_some_and(|tail| tail.contains("refresh-hook-ran")));

    // Exactly one commit, sole parent the previous HEAD, sole path the proposal.
    assert_eq!(fx.rev(&fx.worktree, "HEAD^"), before);
    assert_eq!(
        git_sync(
            &fx.worktree,
            &["rev-list", "--count", &format!("{before}..HEAD")]
        ),
        "1"
    );
    assert_eq!(
        git_sync(
            &fx.worktree,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
        ),
        "openspec/changes/alpha/proposal.md"
    );
    assert_eq!(
        git_sync(&fx.worktree, &["log", "-1", "--format=%s"]),
        format!("Refresh proposal: alpha from {}", &develop[..12])
    );
    assert_eq!(fx.candidate_text(), CORRECTED_PROPOSAL);
    // Base branch, implementation, and every other file untouched.
    assert_eq!(fx.develop(), develop);
    assert_eq!(
        std::fs::read(fx.worktree.join("src/impl.rs")).unwrap(),
        implementation
    );
    assert_eq!(
        std::fs::read(fx.worktree.join("openspec/changes/alpha/tasks.md")).unwrap(),
        tasks
    );
    assert_eq!(fx.status(), "");

    // A second apply with fresh expectations is a truthful no-op.
    let again = apply(&fx.root, &fx.worktree, &fx.request()).await;
    assert_eq!(again.outcome, Some(RefreshOutcome::Unchanged), "{again:?}");
    assert_eq!(
        again.effects,
        RefreshEffects {
            file_written: false,
            staged: false,
            committed: false,
            clean_after: true,
        }
    );
    assert_eq!(fx.head(), after);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_preserves_the_candidate_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new();
    let leaf = fx.worktree.join(proposal_path(CHANGE));
    std::fs::set_permissions(&leaf, std::fs::Permissions::from_mode(0o755)).unwrap();
    git_sync(&fx.worktree, &["commit", "-q", "-am", "make executable"]);

    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;

    assert_eq!(
        report.outcome,
        Some(RefreshOutcome::Refreshed),
        "{report:?}"
    );
    assert_eq!(
        std::fs::metadata(&leaf).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

/// Assert a pre-write refusal left HEAD, the index, and the file untouched.
fn assert_no_effect(fx: &Fixture, report: &RefreshReport, head: &str, status: &str) {
    assert!(!report.effects.file_written, "{report:?}");
    assert!(!report.effects.staged, "{report:?}");
    assert!(!report.effects.committed, "{report:?}");
    assert_eq!(fx.head(), head);
    assert_eq!(fx.status(), status);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_refuses_moved_expectations_without_writing() {
    let fx = Fixture::new();
    let head = fx.head();

    let mut stale_source = fx.request();
    stale_source.expected_source = fx.rev(&fx.root, "develop^");
    let report = apply(&fx.root, &fx.worktree, &stale_source).await;
    assert_eq!(
        report.refusal,
        Some(RefreshReason::RefreshExpectationMismatch)
    );
    assert!(report
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains(&fx.develop())));
    assert_no_effect(&fx, &report, &head, "");

    let mut stale_head = fx.request();
    stale_head.expected_head = fx.rev(&fx.worktree, "HEAD^");
    let report = apply(&fx.root, &fx.worktree, &stale_head).await;
    assert_eq!(
        report.refusal,
        Some(RefreshReason::RefreshExpectationMismatch)
    );
    assert!(report.effects.clean_after);
    assert_no_effect(&fx, &report, &head, "");
    assert_eq!(fx.candidate_text(), STALE_PROPOSAL);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_refuses_every_dirty_variant_without_writing() {
    let fx = Fixture::new();
    let head = fx.head();

    // Untracked, including a path a Conflux-owned exclusion would hide elsewhere.
    write(&fx.worktree, ".cflx/state.json", "{}\n");
    let status = fx.status();
    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;
    assert_eq!(
        report.refusal,
        Some(RefreshReason::CandidateDirty),
        "{report:?}"
    );
    assert!(report
        .detail
        .as_deref()
        .is_some_and(|d| d.contains(".cflx/state.json")));
    assert!(!report.effects.clean_after);
    assert_no_effect(&fx, &report, &head, &status);
    std::fs::remove_dir_all(fx.worktree.join(".cflx")).unwrap();

    // Unstaged.
    write(&fx.worktree, "src/impl.rs", "pub fn edited() {}\n");
    let status = fx.status();
    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;
    assert_eq!(report.refusal, Some(RefreshReason::CandidateDirty));
    assert_no_effect(&fx, &report, &head, &status);

    // Staged.
    git_sync(&fx.worktree, &["add", "src/impl.rs"]);
    let status = fx.status();
    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;
    assert_eq!(report.refusal, Some(RefreshReason::CandidateDirty));
    assert_no_effect(&fx, &report, &head, &status);
    assert_eq!(fx.candidate_text(), STALE_PROPOSAL);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_refuses_an_in_progress_merge() {
    let fx = Fixture::new();
    let head = fx.head();
    let git_dir = PathBuf::from(git_sync(&fx.worktree, &["rev-parse", "--absolute-git-dir"]));
    std::fs::write(git_dir.join("MERGE_HEAD"), format!("{}\n", fx.develop())).unwrap();

    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;

    assert_eq!(
        report.refusal,
        Some(RefreshReason::CandidateOperationInProgress),
        "{report:?}"
    );
    assert!(report
        .detail
        .as_deref()
        .is_some_and(|d| d.contains("MERGE_HEAD")));
    assert_no_effect(&fx, &report, &head, "");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_refuses_unresolvable_and_missing_inputs() {
    let fx = Fixture::new();
    let head = fx.head();

    for source_ref in ["no-such-branch", "--output=/tmp/x", ""] {
        let mut request = fx.request();
        request.source_ref = source_ref.to_string();
        let report = apply(&fx.root, &fx.worktree, &request).await;
        assert_eq!(
            report.refusal,
            Some(RefreshReason::SourceRefUnresolvable),
            "{source_ref}: {report:?}"
        );
        assert_no_effect(&fx, &report, &head, "");
    }

    // Missing source blob: a source commit without the change's proposal.
    git_sync(&fx.root, &["checkout", "-q", "-b", "bare-source"]);
    git_sync(&fx.root, &["rm", "-q", &proposal_path(CHANGE)]);
    git_sync(&fx.root, &["commit", "-q", "-m", "drop proposal"]);
    git_sync(&fx.root, &["checkout", "-q", "develop"]);
    let mut request = fx.request();
    request.source_ref = "bare-source".to_string();
    request.expected_source = fx.rev(&fx.root, "bare-source");
    let report = apply(&fx.root, &fx.worktree, &request).await;
    assert_eq!(report.refusal, Some(RefreshReason::SourceProposalMissing));
    assert_no_effect(&fx, &report, &head, "");

    // Missing candidate blob.
    git_sync(&fx.worktree, &["rm", "-q", &proposal_path(CHANGE)]);
    git_sync(
        &fx.worktree,
        &["commit", "-q", "-m", "drop candidate proposal"],
    );
    let head = fx.head();
    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;
    assert_eq!(
        report.refusal,
        Some(RefreshReason::CandidateProposalMissing)
    );
    assert_no_effect(&fx, &report, &head, "");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_refuses_a_symlinked_proposal_path() {
    let fx = Fixture::new();
    let leaf = fx.worktree.join(proposal_path(CHANGE));
    let outside = fx.worktree.parent().unwrap().join("outside.md");
    std::fs::write(&outside, STALE_PROPOSAL).unwrap();
    std::fs::remove_file(&leaf).unwrap();
    std::os::unix::fs::symlink(&outside, &leaf).unwrap();
    git_sync(&fx.worktree, &["add", "-A"]);
    git_sync(&fx.worktree, &["commit", "-q", "-m", "symlink proposal"]);
    let head = fx.head();

    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;

    assert_eq!(
        report.refusal,
        Some(RefreshReason::UnsafeProposalPath),
        "{report:?}"
    );
    assert_no_effect(&fx, &report, &head, "");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), STALE_PROPOSAL);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_hook_rejection_retains_the_staged_file() {
    let fx = Fixture::new();
    let head = fx.head();
    fx.hook("echo refresh-hook-refused >&2\nexit 1");

    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;

    assert_eq!(
        report.refusal,
        Some(RefreshReason::HookRejected),
        "{report:?}"
    );
    assert_eq!(report.outcome, None);
    assert_eq!(
        report.effects,
        RefreshEffects {
            file_written: true,
            staged: true,
            committed: false,
            clean_after: false,
        }
    );
    assert!(report
        .hook_output_tail
        .as_deref()
        .is_some_and(|tail| tail.contains("refresh-hook-refused")));
    // Nothing was reset or cleaned: the corrected file is still staged.
    assert_eq!(fx.head(), head);
    assert_eq!(report.candidate_head_after.as_deref(), Some(head.as_str()));
    assert_eq!(fx.status(), "M  openspec/changes/alpha/proposal.md");
    assert_eq!(fx.candidate_text(), CORRECTED_PROPOSAL);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_reports_a_hook_that_restaged_another_file() {
    let fx = Fixture::new();
    let head = fx.head();
    fx.hook("echo smuggled > smuggled.txt\ngit add smuggled.txt");

    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;

    assert_eq!(
        report.refusal,
        Some(RefreshReason::RefreshCommitUnexpectedTree),
        "{report:?}"
    );
    assert!(report.effects.committed);
    assert!(report.effects.file_written && report.effects.staged);
    // The commit is left exactly where it landed; nothing was amended or reset.
    let after = fx.head();
    assert_ne!(after, head);
    assert_eq!(report.candidate_head_after.as_deref(), Some(after.as_str()));
    assert_eq!(fx.rev(&fx.worktree, "HEAD^"), head);
    let changed = git_sync(
        &fx.worktree,
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    assert!(changed.contains("smuggled.txt"), "{changed}");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_reports_a_hook_that_left_the_worktree_dirty() {
    let fx = Fixture::new();
    let head = fx.head();
    fx.hook("echo leftover > leftover.txt");

    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;

    assert_eq!(
        report.refusal,
        Some(RefreshReason::HookLeftWorkspaceDirty),
        "{report:?}"
    );
    assert!(report.effects.committed);
    assert!(!report.effects.clean_after);
    assert_eq!(fx.rev(&fx.worktree, "HEAD^"), head);
    assert!(report
        .detail
        .as_deref()
        .is_some_and(|d| d.contains("leftover.txt")));
    assert!(fx.worktree.join("leftover.txt").exists());
}
