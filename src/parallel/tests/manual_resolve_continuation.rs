//! Repository-scoped coverage for continuing a change's own unfinished target merge.
//!
//! The failure these tests pin down is concrete: bounded sequential resolve can
//! exhaust its retries after Git has already staged a conflict-free target
//! merge, and the generic base-dirty preflight then turns the operator's `M`
//! retry — the only advertised recovery control — into another manual deferral.
//!
//! The evidence *is* Git state here, so these are integration-scoped rather than
//! unit tests; the policy itself is exercised against in-memory doubles in
//! `crate::parallel::manual_continuation`. Each fixture is one small temporary
//! repository so they stay in the default suite.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::OrchestratorConfig;
use crate::orchestration::state::{OrchestratorState, ReducerCommand, WaitState};
use crate::parallel::manual_continuation::ManualContinuationAuthorization;
use crate::parallel::merge::MergeAttempt;
use crate::parallel::{MergeResultDisposition, MergeTaskOutcome, ParallelExecutor};

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_ok(cwd: &Path, args: &[&str]) -> String {
    git(cwd, args).unwrap_or_else(|| panic!("git {:?} failed in {}", args, cwd.display()))
}

/// A target repository whose `MERGE_HEAD` is a change's own conflict-free merge.
struct UnfinishedMerge {
    _dir: tempfile::TempDir,
    root: PathBuf,
    workspace: PathBuf,
}

impl UnfinishedMerge {
    fn merge_head(&self) -> Option<String> {
        git(&self.root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"])
    }

    fn head(&self) -> String {
        git_ok(&self.root, &["rev-parse", "HEAD"])
    }

    fn status(&self) -> String {
        git_ok(&self.root, &["status", "--porcelain"])
    }

    /// Whether the stand-in resolve agent ran, recorded as a Git tag so the
    /// probe itself cannot be confused with ordinary worktree dirt.
    fn agent_ran(&self) -> bool {
        git(
            &self.root,
            &["rev-parse", "-q", "--verify", "refs/tags/agent-ran"],
        )
        .is_some()
    }
}

/// Build a target whose `MERGE_HEAD` is `change_id`'s archive branch.
///
/// The base still holds the live change directory and the branch archives it,
/// so the staged merge result both adds and deletes paths — the shape a real
/// post-archive integration has.
fn unfinished_target_merge(change_id: &str) -> Option<UnfinishedMerge> {
    let dir = tempfile::tempdir().ok()?;
    let root = dir.path().join("repo");
    std::fs::create_dir_all(&root).ok()?;
    git(&root, &["init", "-b", "main"])?;
    git(&root, &["config", "user.email", "test@example.com"])?;
    git(&root, &["config", "user.name", "Test User"])?;
    git(&root, &["config", "commit.gpgsign", "false"])?;
    std::fs::write(root.join("README.md"), "# base\n").ok()?;
    let live = root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&live).ok()?;
    std::fs::write(live.join("proposal.md"), "# live\n").ok()?;
    git(&root, &["add", "-A"])?;
    git(&root, &["commit", "-m", "Initial commit"])?;

    let workspace = dir.path().join("workspaces").join(change_id);
    std::fs::create_dir_all(workspace.parent()?).ok()?;
    git(
        &root,
        &[
            "worktree",
            "add",
            "-b",
            change_id,
            workspace.to_str()?,
            "HEAD",
        ],
    )?;
    std::fs::remove_dir_all(workspace.join("openspec/changes").join(change_id)).ok()?;
    let archived = workspace.join("openspec/changes/archive").join(change_id);
    std::fs::create_dir_all(&archived).ok()?;
    std::fs::write(archived.join("proposal.md"), "# archived\n").ok()?;
    git(&workspace, &["add", "-A"])?;
    git(
        &workspace,
        &["commit", "-m", &format!("Archive: {}", change_id)],
    )?;

    // Exactly the state bounded resolve leaves behind: a conflict-free merge
    // that was never committed.
    git(&root, &["merge", "--no-ff", "--no-commit", change_id])?;

    Some(UnfinishedMerge {
        _dir: dir,
        root,
        workspace,
    })
}

/// A config whose "resolve agent" is a deterministic shell command.
///
/// The command-queue pacing exists for real agent processes; paying for it here
/// would push these tests out of the default suite.
fn config(resolve_command: &str, workspace_base: &Path) -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply".to_string()),
        archive_command: Some("echo archive".to_string()),
        resolve_command: Some(resolve_command.to_string()),
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_queue_retry_if_duration_under_secs: Some(0),
        ..Default::default()
    }
}

/// The commit that finishes an in-progress target merge, as the classifier's
/// `target_merge_unfinished` guidance describes it.
fn commit_final_merge(change_id: &str) -> String {
    format!("git commit --no-edit -m 'Merge change: {}'", change_id)
}

/// A resolve command that only records that it ran.
const AGENT_PROBE: &str = "git tag agent-ran";

/// The executor plus the event receiver that must outlive it.
///
/// These tests assert on repository evidence rather than on the event stream,
/// but dropping the receiver would turn every emission into a send failure, so
/// the caller binds it for the duration of the test.
type ExecutorUnderTest = (
    ParallelExecutor,
    tokio::sync::mpsc::Receiver<crate::events::ExecutionEvent>,
);

fn executor(fixture: &UnfinishedMerge, resolve_command: &str) -> ExecutorUnderTest {
    let workspace_base = fixture
        .workspace
        .parent()
        .expect("workspace base")
        .to_path_buf();
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    (
        ParallelExecutor::new(
            fixture.root.clone(),
            config(resolve_command, &workspace_base),
            Some(tx),
        ),
        rx,
    )
}

fn deferral(attempt: MergeAttempt) -> String {
    match attempt {
        MergeAttempt::Deferred(deferred) => {
            assert!(
                !deferred.auto_resumable,
                "a refused continuation is operator work, not an auto-resumable wait: {}",
                deferred.reason
            );
            deferred.reason
        }
        other => panic!("expected a deferral, got {:?}", other),
    }
}

/// Run one authorized retry against `fixture` and return its deferral reason,
/// asserting no agent ran and no Git state moved.
async fn refused_retry(
    fixture: &UnfinishedMerge,
    revisions: &[String],
    change_ids: &[String],
) -> String {
    let (executor, _events) = executor(fixture, AGENT_PROBE);
    let authorization = ManualContinuationAuthorization::new("alpha");
    let head_before = fixture.head();
    let merge_head_before = fixture.merge_head();
    let status_before = fixture.status();

    let archive_paths: Vec<PathBuf> = revisions
        .iter()
        .map(|_| fixture.workspace.clone())
        .collect();
    let attempt = executor
        .attempt_merge(revisions, change_ids, &archive_paths, Some(&authorization))
        .await
        .expect("a refused continuation is a deferral, not a base-lane failure");

    let reason = deferral(attempt);
    assert!(
        !fixture.agent_ran(),
        "no agent may start on a refused retry"
    );
    assert_eq!(fixture.head(), head_before, "HEAD must not move");
    assert_eq!(
        fixture.merge_head(),
        merge_head_before,
        "the in-progress merge must be preserved, not aborted"
    );
    assert_eq!(
        fixture.status(),
        status_before,
        "worktree state must be preserved"
    );
    reason
}

#[tokio::test]
async fn manual_resolve_retry_ordinary_attempt_still_defers_on_merge_head() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;
    let (executor, _events) = executor(&fixture, AGENT_PROBE);

    let attempt = executor
        .attempt_merge(
            &["alpha".to_string()],
            &["alpha".to_string()],
            std::slice::from_ref(&fixture.workspace),
            None,
        )
        .await
        .expect("an unauthorized attempt defers");

    assert!(
        deferral(attempt).contains("Merge in progress (MERGE_HEAD exists)"),
        "an ordinary scheduled attempt keeps the unchanged generic dirty preflight"
    );
    assert!(!fixture.agent_ran());
    assert!(fixture.merge_head().is_some());
}

#[tokio::test]
async fn manual_resolve_retry_continues_its_own_unfinished_target_merge() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;
    let (executor, _events) = executor(&fixture, &commit_final_merge("alpha"));
    let authorization = ManualContinuationAuthorization::new("alpha");

    let attempt = executor
        .attempt_merge(
            &["alpha".to_string()],
            &["alpha".to_string()],
            std::slice::from_ref(&fixture.workspace),
            Some(&authorization),
        )
        .await
        .expect("an authorized retry reaches sequential resolve");

    assert!(
        matches!(attempt, MergeAttempt::Merged { .. }),
        "the retry must continue the merge instead of deferring: {:?}",
        attempt
    );
    assert!(
        authorization.is_consumed(),
        "the admitted dispatch consumes its one-dispatch authorization"
    );
    assert!(
        fixture.merge_head().is_none(),
        "a completed continuation clears MERGE_HEAD"
    );
    assert_eq!(
        git_ok(&fixture.root, &["log", "-1", "--pretty=%s"]),
        "Merge change: alpha",
        "the per-change final integration keeps its exact subject"
    );
    assert!(
        fixture.status().is_empty(),
        "a completed continuation leaves the target clean: {}",
        fixture.status()
    );
}

#[tokio::test]
async fn manual_resolve_retry_refuses_foreign_merge_head() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;

    // The retry is for `alpha`, but the batch admits a branch that owns nothing
    // in progress: MERGE_HEAD is provably not this dispatch's own merge.
    git_ok(&fixture.root, &["branch", "unrelated", "HEAD"]);
    let reason = refused_retry(&fixture, &["unrelated".to_string()], &["alpha".to_string()]).await;

    assert!(
        reason.contains("matches no admitted branch tip"),
        "unexpected refusal: {}",
        reason
    );
}

#[tokio::test]
async fn manual_resolve_retry_refuses_ambiguous_merge_head() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;

    git_ok(&fixture.root, &["branch", "alpha-duplicate", "alpha"]);
    let reason = refused_retry(
        &fixture,
        &["alpha".to_string(), "alpha-duplicate".to_string()],
        &["alpha".to_string(), "beta".to_string()],
    )
    .await;

    assert!(
        reason.contains("ownership is ambiguous"),
        "unexpected refusal: {}",
        reason
    );
}

#[tokio::test]
async fn manual_resolve_retry_refuses_unresolved_conflicts() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;

    // Re-create the merge as a conflicting one: both sides changed README.md.
    git_ok(&fixture.root, &["merge", "--abort"]);
    std::fs::write(fixture.root.join("README.md"), "# base moved\n").expect("write base");
    git_ok(&fixture.root, &["commit", "-am", "Base moves"]);
    std::fs::write(fixture.workspace.join("README.md"), "# branch moved\n").expect("write branch");
    git_ok(&fixture.workspace, &["commit", "-am", "Branch moves"]);
    assert!(
        git(&fixture.root, &["merge", "--no-ff", "--no-commit", "alpha"]).is_none(),
        "fixture must leave a conflicted merge"
    );

    let reason = refused_retry(&fixture, &["alpha".to_string()], &["alpha".to_string()]).await;

    assert!(
        reason.contains("unresolved conflicts"),
        "unexpected refusal: {}",
        reason
    );
}

#[tokio::test]
async fn manual_resolve_retry_refuses_unrelated_staged_change() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;

    std::fs::write(fixture.root.join("unrelated.txt"), "not mine\n").expect("write unrelated");
    git_ok(&fixture.root, &["add", "unrelated.txt"]);

    let reason = refused_retry(&fixture, &["alpha".to_string()], &["alpha".to_string()]).await;

    assert!(
        reason.contains("staged content the merge") && reason.contains("unrelated.txt"),
        "unexpected refusal: {}",
        reason
    );
}

#[tokio::test]
async fn manual_resolve_retry_refuses_unrelated_unstaged_change() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;

    std::fs::write(fixture.root.join("README.md"), "# edited by hand\n").expect("edit readme");

    let reason = refused_retry(&fixture, &["alpha".to_string()], &["alpha".to_string()]).await;

    assert!(
        reason.contains("does not match the index"),
        "unexpected refusal: {}",
        reason
    );
}

#[tokio::test]
async fn manual_resolve_retry_refuses_conflicting_untracked_path() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;

    // The merge removes the live change directory; recreating it on disk is a
    // deviation from `HEAD` the merge cannot account for.
    let resurrected = fixture.root.join("openspec/changes/alpha");
    std::fs::create_dir_all(&resurrected).expect("resurrect live change");
    std::fs::write(resurrected.join("proposal.md"), "# resurrected\n").expect("write proposal");

    let reason = refused_retry(&fixture, &["alpha".to_string()], &["alpha".to_string()]).await;

    assert!(
        reason.contains("Untracked paths conflict"),
        "unexpected refusal: {}",
        reason
    );
}

#[tokio::test]
async fn manual_resolve_retry_authorization_survives_occupied_base_lane() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;
    let (executor, _events) = executor(&fixture, AGENT_PROBE);
    executor
        .auto_resolve_count
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let authorization = ManualContinuationAuthorization::new("alpha");

    let attempt = executor
        .attempt_merge(
            &["alpha".to_string()],
            &["alpha".to_string()],
            std::slice::from_ref(&fixture.workspace),
            Some(&authorization),
        )
        .await
        .expect("occupancy is a deferral");

    match attempt {
        MergeAttempt::Deferred(deferred) => assert!(
            deferred.auto_resumable,
            "an occupied lane stays auto-resumable: {}",
            deferred.reason
        ),
        other => panic!("an occupied lane must defer, got {:?}", other),
    }
    assert!(
        !authorization.is_consumed(),
        "a dispatch that never owned the lane must not consume its authorization"
    );
    assert!(!fixture.agent_ran());
}

#[tokio::test]
async fn manual_resolve_retry_authorization_is_not_sticky() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;
    let (executor, _events) = executor(&fixture, AGENT_PROBE);
    let authorization = ManualContinuationAuthorization::new("alpha");
    assert!(
        authorization.consume_for("alpha"),
        "fixture consumes it once"
    );

    let attempt = executor
        .attempt_merge(
            &["alpha".to_string()],
            &["alpha".to_string()],
            std::slice::from_ref(&fixture.workspace),
            Some(&authorization),
        )
        .await
        .expect("a spent authorization still defers rather than failing");

    assert!(
        deferral(attempt).contains("Merge in progress (MERGE_HEAD exists)"),
        "a later attempt observes the unchanged generic dirty preflight"
    );
    assert!(!fixture.agent_ran());
    assert!(fixture.merge_head().is_some());
}

/// The reducer permission an explicit `M` carries, promoted onto the lane.
async fn manual_retry_state(
    change_id: &str,
    other: &str,
) -> std::sync::Arc<tokio::sync::RwLock<OrchestratorState>> {
    let shared = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec![change_id.to_string(), other.to_string()],
        0,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&crate::events::ExecutionEvent::MergeDeferred {
            change_id: change_id.to_string(),
            reason: "bounded resolve exhausted".to_string(),
            auto_resumable: false,
        });
        guard.apply_command(ReducerCommand::ResolveMerge(change_id.to_string()));
        assert!(
            guard.has_manual_resolve_retry(change_id),
            "explicit operator intent is what grants the permission"
        );
        assert_eq!(
            guard.promote_next_base_mutating_lane_waiter(),
            Some((change_id.to_string(), WaitState::ResolveWait))
        );
    }
    shared
}

#[tokio::test]
async fn manual_resolve_retry_completes_the_merge_wait_lifecycle() {
    let Some(fixture) = unfinished_target_merge("alpha") else {
        return;
    };
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;
    let (mut executor, _events) = executor(&fixture, &commit_final_merge("alpha"));
    let shared = manual_retry_state("alpha", "beta").await;
    executor.set_shared_orchestrator_state(shared.clone());
    executor.resolve_wait_changes.insert("alpha".to_string());

    let authorization = std::sync::Arc::new(ManualContinuationAuthorization::new("alpha"));
    let outcome = executor
        .retry_deferred_merges_for(vec!["alpha".to_string()], Some(authorization))
        .await;

    assert_eq!(outcome, MergeTaskOutcome::Merged);
    assert_eq!(
        outcome.disposition(),
        MergeResultDisposition::Merged,
        "recovery is change-scoped; it never aborts the run"
    );
    assert!(fixture.merge_head().is_none());
    assert_eq!(
        git_ok(&fixture.root, &["log", "-1", "--pretty=%s"]),
        "Merge change: alpha"
    );

    let guard = shared.read().await;
    assert_eq!(guard.display_status("alpha"), "merged");
    assert!(
        !guard.has_manual_resolve_retry("alpha"),
        "the consumed permission must not survive its dispatch"
    );
    assert_eq!(
        guard.display_status("beta"),
        "not queued",
        "another change is untouched by one change's recovery"
    );
    assert!(guard.global_invariants_hold());
}
