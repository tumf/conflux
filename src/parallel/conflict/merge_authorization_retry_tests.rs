//! Retry-loop behaviour of the sequential merge-authorization gate.
//!
//! These run the real `resolve_merges_with_retry` loop against a real Git
//! repository and a real (scripted) resolve command, because what is under test
//! is exactly what an in-memory classifier test cannot show: how many agent
//! attempts a withheld merge costs, and what the repository looks like
//! afterwards. The scripted resolve command records every invocation, so
//! "another agent was not started" is a counted fact rather than an inference.

use super::fixtures::MockWorkspaceManager;
use super::{resolve_merges_with_retry, ResolveFailure, ResolveMergesWithRetryArgs};
use crate::parallel::resolve_state::SequentialMergeItem;
use crate::parallel::types::ResolveFailureClassification;
use crate::vcs::WorkspaceManager;
use std::path::{Path, PathBuf};

/// One prepared sequential batch: a target repository, one change worktree, and
/// a resolve command that counts its own invocations.
struct Harness {
    _dir: tempfile::TempDir,
    root: PathBuf,
    worktree: PathBuf,
    base_revision: String,
    resolve_command: String,
    invocations: PathBuf,
    items: Vec<SequentialMergeItem>,
}

impl Harness {
    /// Agent attempts actually launched so far.
    fn attempts(&self) -> usize {
        std::fs::read_to_string(&self.invocations)
            .map(|text| text.lines().filter(|line| !line.is_empty()).count())
            .unwrap_or(0)
    }

    fn head(&self, dir: &Path) -> String {
        git(dir, &["rev-parse", "HEAD"])
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|error| panic!("git {:?} in {}: {}", args, dir.display(), error));
    assert!(
        output.status.success(),
        "git {:?} in {} failed: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, contents).expect("write file");
}

/// A task list recording `completed` of `total` tasks done.
fn tasks_markdown(completed: u32, total: u32) -> String {
    let mut content = String::from("## Implementation Tasks\n\n");
    for index in 0..total {
        let box_ = if index < completed { "x" } else { " " };
        content.push_str(&format!("- [{}] Task {}\n", box_, index + 1));
    }
    content
}

/// Build a batch whose single change is archived on its own branch with
/// `completed`/`total` tasks recorded, and whose branch still needs a pre-sync.
///
/// `script_body` is the shell the scripted resolve command runs after recording
/// its invocation; it stands in for whatever an agent would do.
fn harness(completed: u32, total: u32, script_body: &str) -> Harness {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let root = dir.path().join("repo");
    std::fs::create_dir_all(&root).expect("repo dir");

    git(&root, &["init", "--initial-branch=main"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    write(&root.join("README.md"), "# base\n");
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "Base"]);
    let base_revision = git(&root, &["rev-parse", "HEAD"]);

    // The change branch: archived, with its own committed task list.
    let worktree = dir.path().join("wt-change-a");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-b",
            "ws-change-a",
            worktree.to_str().expect("worktree path"),
            "HEAD",
        ],
    );
    let archive = worktree.join("openspec/changes/archive/change-a");
    write(&archive.join("proposal.md"), "# archived change-a\n");
    write(&archive.join("tasks.md"), &tasks_markdown(completed, total));
    git(&worktree, &["add", "-A"]);
    git(&worktree, &["commit", "-m", "Archive: change-a"]);

    // The target moves on, so a pre-sync is genuinely required first.
    write(&root.join("other.txt"), "target advance\n");
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "Target advance"]);

    // `$WT` and `$ROOT` are the two locations a resolve agent is given, so the
    // scripted stand-in gets exactly those and nothing else.
    let invocations = dir.path().join("invocations.log");
    let script = dir.path().join("resolve.sh");
    write(
        &script,
        &format!(
            "#!/bin/sh\nset -e\nWT={wt}\nROOT={root}\necho attempt >> {invocations}\n{body}\n",
            wt = worktree.display(),
            root = root.display(),
            invocations = invocations.display(),
            body = script_body
        ),
    );

    let items = SequentialMergeItem::batch(
        &["ws-change-a".to_string()],
        &["change-a".to_string()],
        std::slice::from_ref(&worktree),
    )
    .expect("well-formed batch");

    Harness {
        _dir: dir,
        root,
        worktree,
        base_revision,
        resolve_command: format!("sh {}", script.display()),
        invocations,
        items,
    }
}

async fn run(harness: &Harness, max_retries: u32) -> std::result::Result<(), ResolveFailure> {
    let manager = MockWorkspaceManager::new(vec![]).with_repo_root(harness.root.clone());
    let config = crate::config::OrchestratorConfig {
        resolve_command: Some(harness.resolve_command.clone()),
        ..Default::default()
    };
    let ai_runner = crate::ai_command_runner::AiCommandRunner::for_run(
        &config,
        std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        crate::ai_command_runner::RunCommandScope::new(),
    );

    resolve_merges_with_retry(ResolveMergesWithRetryArgs {
        workspace_manager: &manager as &dyn WorkspaceManager,
        config: &config,
        event_tx: &None,
        items: &harness.items,
        target_branch: "main",
        base_revision: &harness.base_revision,
        max_retries,
        ai_runner,
        auto_resolve_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        publication_owns_completion: false,
    })
    .await
}

fn assert_withheld(result: std::result::Result<(), ResolveFailure>, expected_attempts: u32) {
    match result {
        Err(ResolveFailure::Exhausted {
            attempts,
            classification,
            detail,
        }) => {
            assert_eq!(
                classification,
                ResolveFailureClassification::EvidenceWithheld,
                "a withheld merge leaves through the evidence-withheld path: {}",
                detail
            );
            assert_eq!(attempts, expected_attempts, "detail: {}", detail);
            assert!(
                detail.contains("merge_not_authorized"),
                "the operator must be told why: {}",
                detail
            );
            for forbidden in ["git merge", "Merge change:", "git commit"] {
                assert!(
                    !detail.contains(forbidden),
                    "withheld guidance must not contain '{}': {}",
                    forbidden,
                    detail
                );
            }
        }
        other => panic!("expected a withheld merge, got {:?}", other),
    }
}

/// Pre-syncing in the change worktree, exactly as the resolve agent would.
const PRESYNC_SCRIPT: &str = r#"
git -C "$WT" merge --no-ff -m "Pre-sync base into change-a" main
"#;

/// Pre-syncing and then integrating, the complete green path.
const PRESYNC_AND_MERGE_SCRIPT: &str = r#"
git -C "$WT" merge --no-ff -m "Pre-sync base into change-a" main
git -C "$ROOT" merge --no-ff -m "Merge change: change-a" ws-change-a
"#;

#[tokio::test]
async fn unchanged_incomplete_evidence_never_starts_a_resolve_agent() {
    // Pre-sync is already done, so the only thing left would be the final merge.
    let harness = harness(6, 7, "true");
    git(
        &harness.worktree,
        &[
            "merge",
            "--no-ff",
            "-m",
            "Pre-sync base into change-a",
            "main",
        ],
    );
    let target_head = harness.head(&harness.root);
    let branch_tip = harness.head(&harness.worktree);

    let result = run(&harness, 3).await;

    assert_withheld(result, 0);
    assert_eq!(
        harness.attempts(),
        0,
        "an unfinished change must not cost a single agent attempt"
    );
    assert_eq!(
        harness.head(&harness.root),
        target_head,
        "the base must be untouched"
    );
    assert_eq!(
        harness.head(&harness.worktree),
        branch_tip,
        "the change worktree must be untouched"
    );
}

#[tokio::test]
async fn a_resolved_presync_does_not_earn_a_second_attempt_at_the_merge() {
    // The first attempt is legitimate work — the pre-sync is genuinely missing.
    // Completing it must not turn into authorization for the final merge, and
    // must not hand the same batch to another agent.
    let harness = harness(6, 7, PRESYNC_SCRIPT);
    let target_head = harness.head(&harness.root);

    let result = run(&harness, 3).await;

    assert_withheld(result, 1);
    assert_eq!(
        harness.attempts(),
        1,
        "exactly one attempt: the pre-sync. The withheld merge starts no second agent"
    );
    assert_eq!(
        harness.head(&harness.root),
        target_head,
        "resolving a pre-sync must not advance the base"
    );
    assert_eq!(
        git(&harness.root, &["log", "--oneline", "main", "--format=%s"])
            .lines()
            .filter(|subject| subject.contains("Merge change: change-a"))
            .count(),
        0,
        "no final merge commit may exist for an unfinished change"
    );
}

#[tokio::test]
async fn complete_tasks_still_reach_a_green_sequential_merge() {
    let harness = harness(7, 7, PRESYNC_AND_MERGE_SCRIPT);

    let result = run(&harness, 3).await;

    assert!(
        result.is_ok(),
        "a complete change keeps the existing merge path: {:?}",
        result
    );
    assert_eq!(
        harness.attempts(),
        1,
        "the green path still costs exactly one attempt"
    );
    assert_eq!(
        git(&harness.root, &["log", "--format=%s", "main"])
            .lines()
            .filter(|subject| *subject == "Merge change: change-a")
            .count(),
        1,
        "the exact per-change final merge commit must exist"
    );
}
