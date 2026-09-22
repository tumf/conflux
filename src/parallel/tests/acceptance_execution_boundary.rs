//! End-to-end regressions for the runtime-owned Acceptance execution boundary.
//!
//! These drive `execute_acceptance_in_workspace` against a real Git worktree and
//! two deliberately dumb reviewers:
//!
//! * a **canonical reviewer** that emits a verdict and asks for nothing else,
//!   used to prove that the runtime — not the reviewer — ran the declared gates;
//! * a **non-terminating reviewer** that never exits, used to prove the shared
//!   review deadline terminates it, reaps it, and settles as a typed
//!   non-resumable `stalled` hold rather than the generic runtime-limit `error`.
//!
//! Every reviewer writes a marker file before doing anything else, so "the
//! reviewer never started" is an observable fact rather than an inference. That
//! matters: most of the claims here are about work that must *not* happen.
//!
//! The review budget is scoped to milliseconds for the deadline tests. The
//! production partition is whole seconds and its smallest positive work budget
//! is one second, which is a fine bound for an operator and a poor one for a
//! suite with a one-second total runtime target.

use std::sync::Arc;

use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::Mutex;

use super::support::{create_test_config_with, init_git_repo, TestAssertionExt};
use crate::agent::AgentRunner;
use crate::ai_command_runner::AiCommandRunner;
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::default_retry_patterns;
use crate::config::OrchestratorConfig;
use crate::orchestration::acceptance::execution_manifest::{
    AcceptanceHoldCategory, ManifestStore, ManifestTerminal,
};
use crate::orchestration::AcceptanceResult;
use crate::parallel::executor::{
    execute_acceptance_in_workspace, scoped_review_budget_millis_for_test,
};

const CHANGE: &str = "change-a";
const REVIEWER_MARKER: &str = "reviewer-started.txt";

/// A proposal whose only eligible declaration is one repository-local
/// change-blocking gate running `command`.
fn proposal_with_gate(command: &str) -> String {
    format!(
        "---\n\
verifications:\n\
\x20 - id: focused-gate\n\
\x20   requirement: the focused gate passes\n\
\x20   phase: pre-integration\n\
\x20   owner: conflux\n\
\x20   trigger: pull-request-validation\n\
\x20   automation: openspec/changes/{CHANGE}/proposal.md\n\
\x20   evidence: {command}\n\
\x20   rerun: {command}\n\
\x20   prerequisites: []\n\
\x20   execution_class: repository-local\n\
\x20   completion_role: change-blocking\n\
---\n\
# Change\n"
    )
}

/// A proposal that declares nothing eligible: an empty blocking allowlist.
const PROPOSAL_WITHOUT_GATES: &str = "---\nchange_type: implementation\n---\n# Change\n";

async fn workspace_with(proposal: &str) -> TempDir {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;
    let change_dir = repo_root.path().join("openspec/changes").join(CHANGE);
    std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(change_dir.join("proposal.md"), proposal).or_fail("unexpected error");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n",
    )
    .or_fail("unexpected error");
    commit_all(repo_root.path(), "Apply: change-a").await;
    repo_root
}

async fn commit_all(repo_root: &std::path::Path, message: &str) {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
}

/// A reviewer that records that it started, emits a canonical verdict, and asks
/// for nothing else. It runs no verification of its own, which is the point:
/// whatever gate evidence exists came from the runtime.
fn canonical_reviewer() -> String {
    format!("sh -c 'touch {REVIEWER_MARKER}; printf \"ACCEPTANCE: PASS\\n\"'")
}

/// A reviewer that records that it started and then never terminates.
fn non_terminating_reviewer() -> String {
    format!("sh -c 'touch {REVIEWER_MARKER}; while true; do sleep 1; done'")
}

fn config_with_reviewer(command: String) -> OrchestratorConfig {
    create_test_config_with(OrchestratorConfig {
        acceptance_command: Some(command),
        ..Default::default()
    })
}

fn runner(acceptance_max_runtime_secs: u64) -> AiCommandRunner {
    AiCommandRunner::new(
        CommandQueueConfig {
            acceptance_max_runtime_secs,
            stagger_delay_ms: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_error_patterns: default_retry_patterns(),
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        },
        Arc::new(Mutex::new(None)),
    )
}

async fn run_acceptance(
    repo_root: &std::path::Path,
    config: &OrchestratorConfig,
    ai_runner: &AiCommandRunner,
) -> (AcceptanceResult, u32) {
    let mut agent = AgentRunner::new(config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));
    execute_acceptance_in_workspace(
        CHANGE,
        repo_root,
        &mut agent,
        None,
        None,
        ai_runner,
        config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
        crate::orchestration::acceptance::AcceptanceCommandMode::Normal,
    )
    .await
    .or_fail("acceptance must return a typed result")
}

fn reviewer_started(repo_root: &std::path::Path) -> bool {
    repo_root.join(REVIEWER_MARKER).exists()
}

// ============================================================================
// The runtime owns declared gate execution
// ============================================================================

/// The declared gate ran under runtime supervision, a bound evidence sidecar
/// exists for it, and the reviewer — which ran no verification at all — still
/// produced a verdict. Nothing about the verdict depended on reviewer-owned
/// command execution.
#[tokio::test]
async fn acceptance_execution_boundary_runtime_executes_declared_gates() {
    let repo_root = workspace_with(&proposal_with_gate("sh -c true")).await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = run_acceptance(repo_root.path(), &config, &runner(3600)).await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "a passing declared gate plus a canonical verdict is a PASS, got {result:?}"
    );
    assert!(
        reviewer_started(repo_root.path()),
        "the semantic reviewer must still run"
    );
    assert!(
        repo_root
            .path()
            .join(".cflx/verification-evidence/focused-gate.json")
            .is_file(),
        "the runtime must write bound evidence for the gate it supervised"
    );
    let manifest = ManifestStore::new(repo_root.path().to_path_buf())
        .load(CHANGE)
        .expect("the manifest must be persisted before the reviewer starts");
    assert_eq!(
        manifest
            .gates
            .iter()
            .map(|gate| gate.verification_id.as_str())
            .collect::<Vec<_>>(),
        vec!["focused-gate"],
        "the manifest is the complete blocking allowlist"
    );
    assert_eq!(manifest.candidate_commit_oid.len(), 40);
    assert!(manifest.review_range.contains(".."));
}

/// A failing declared gate decides the verdict by itself: the reviewer is never
/// started, so no budget is spent rediscovering what the runtime already knows.
#[tokio::test]
async fn acceptance_execution_boundary_failing_gate_fails_without_starting_a_reviewer() {
    let repo_root = workspace_with(&proposal_with_gate("sh -c false")).await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, iteration) = run_acceptance(repo_root.path(), &config, &runner(3600)).await;

    match result {
        AcceptanceResult::Fail { findings } => {
            assert!(
                findings
                    .iter()
                    .any(|finding| finding.text().contains("focused-gate")),
                "the failing gate must be the finding's evidence: {findings:?}"
            );
        }
        other => panic!("a failing declared gate must FAIL the change, got {other:?}"),
    }
    assert!(iteration > 0, "a gate-decided FAIL records a real attempt");
    assert!(
        !reviewer_started(repo_root.path()),
        "the reviewer must not be started once the verdict is already determined"
    );
}

/// A change that declares nothing eligible still reaches a verdict. The empty
/// allowlist is a boundary, not a reason to hold.
#[tokio::test]
async fn acceptance_execution_boundary_empty_allowlist_still_reaches_a_verdict() {
    let repo_root = workspace_with(PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = run_acceptance(repo_root.path(), &config, &runner(3600)).await;

    assert!(matches!(result, AcceptanceResult::Pass), "{result:?}");
    assert!(reviewer_started(repo_root.path()));
    let manifest = ManifestStore::new(repo_root.path().to_path_buf())
        .load(CHANGE)
        .expect("even a gateless change gets a manifest");
    assert!(!manifest.has_gates());
}

/// A workspace that is not a Git working tree has no candidate to freeze and no
/// evidence to bind, so it gets no manifest — not a hold. Acceptance runs there
/// exactly as it did before the boundary existed.
///
/// The distinction matters: collapsing "never a repository" into "repository
/// unreadable" would turn every non-repository Acceptance into a permanent hold
/// nobody can clear.
#[tokio::test]
async fn acceptance_execution_boundary_non_repository_workspace_gets_no_manifest() {
    let workspace = TempDir::new().or_fail("unexpected error");
    let change_dir = workspace.path().join("openspec/changes").join(CHANGE);
    std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(
        change_dir.join("proposal.md"),
        proposal_with_gate("sh -c true"),
    )
    .or_fail("unexpected error");
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = run_acceptance(workspace.path(), &config, &runner(3600)).await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "a non-repository workspace must keep its pre-boundary behaviour, got {result:?}"
    );
    assert!(reviewer_started(workspace.path()));
    assert!(
        ManifestStore::new(workspace.path().to_path_buf())
            .load(CHANGE)
            .is_err(),
        "no manifest may be written for a workspace with no candidate revision"
    );
}

// ============================================================================
// The shared review deadline
// ============================================================================

/// The central regression: a reviewer that never terminates is terminated by
/// the runtime at the review deadline, its process group is reaped, and the
/// invocation settles as a non-resumable `stalled` execution hold with a stable
/// category — not as the generic runtime-limit `error`, and not into
/// missing-verdict retry.
#[tokio::test]
async fn acceptance_execution_boundary_non_terminating_reviewer_settles_as_stalled() {
    let repo_root = workspace_with(PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(non_terminating_reviewer());
    // A generous absolute limit, so what fires is the review deadline rather
    // than the outer runtime limit this change exists to stop reaching.
    let ai_runner = runner(3600);

    let (result, iteration) = scoped_review_budget_millis_for_test(
        150,
        run_acceptance(repo_root.path(), &config, &ai_runner),
    )
    .await;

    let hold = match result {
        AcceptanceResult::ExecutionHold { hold } => hold,
        other => panic!("a verdictless reviewer must settle as an execution hold, got {other:?}"),
    };
    assert!(
        matches!(
            hold.category,
            AcceptanceHoldCategory::ReviewDeadlineExhausted
                | AcceptanceHoldCategory::ReviewCleanupUnproven
        ),
        "unexpected hold category {:?}",
        hold.category
    );
    assert!(reviewer_started(repo_root.path()));
    assert_eq!(
        iteration, 0,
        "a bounded hold is not a verdict and consumes no attempt number"
    );
    assert!(!hold.permits_retry());

    let blocker = hold.to_stalled_blocker(CHANGE);
    assert_eq!(blocker.phase, "acceptance");
    assert!(!blocker.resumable);
    assert!(blocker.unblock_condition.is_none());
    assert!(blocker.worktree_preserved);

    // The terminal record is persisted in the worktree, which is what a restart
    // re-derives the refusal from.
    let stored = ManifestStore::new(repo_root.path().to_path_buf())
        .load(CHANGE)
        .expect("the terminal manifest must be persisted");
    let fingerprint = stored.fingerprint();
    let terminal = stored
        .terminal
        .or_fail("the hold must be recorded on the manifest");
    assert_eq!(terminal.hold_category, Some(hold.category));
    assert_eq!(terminal.fingerprint, fingerprint);
}

/// The result classifies as a hold rather than as a runtime limit, so it never
/// reaches verdict routing, escalation, or automatic re-dispatch.
#[tokio::test]
async fn acceptance_execution_boundary_hold_is_not_a_runtime_limit_or_a_verdict() {
    let repo_root = workspace_with(PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(non_terminating_reviewer());
    let ai_runner = runner(3600);

    let (result, _) = scoped_review_budget_millis_for_test(
        150,
        run_acceptance(repo_root.path(), &config, &ai_runner),
    )
    .await;

    assert!(result.is_execution_hold());
    assert!(!result.is_runtime_limit(), "the outer limit must not fire");
    assert!(!result.is_canonical_verdict());
    assert!(
        !result.permits_acceptance_retry(),
        "a bounded hold must not be re-dispatched automatically"
    );
    assert!(
        crate::orchestration::acceptance::classify_invalid_acceptance_result(&result).is_none(),
        "a boundary decision is not an invalid reviewer response"
    );
}

/// A reviewer that emits its verdict before the deadline keeps it, even though
/// the runtime terminates the still-open process afterwards. Terminating a
/// process that already answered is finalization, not failure.
#[tokio::test]
async fn acceptance_execution_boundary_verdict_before_the_deadline_is_kept() {
    let repo_root = workspace_with(PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(format!(
        "sh -c 'touch {REVIEWER_MARKER}; printf \"ACCEPTANCE: PASS\\n\"; while true; do sleep 1; done'"
    ));
    let ai_runner = runner(3600);

    let (result, iteration) = scoped_review_budget_millis_for_test(
        2_000,
        crate::parallel::executor::scoped_verdict_grace_secs_for_test(
            1,
            run_acceptance(repo_root.path(), &config, &ai_runner),
        ),
    )
    .await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "a verdict emitted before the deadline must stand, got {result:?}"
    );
    assert!(iteration > 0);
}

// ============================================================================
// Unchanged-input refusal, derived from the workspace
// ============================================================================

/// Record a terminal hold against the manifest the current workspace produces.
async fn record_hold(repo_root: &std::path::Path, config: &OrchestratorConfig) -> String {
    let manifest =
        crate::orchestration::acceptance::manifest_builder::build_manifest_for_workspace(
            &crate::orchestration::acceptance::verification_evidence::GitRepositoryFacts,
            repo_root,
            CHANGE,
            config.get_accept_skill(),
            Some("main"),
            3600,
        )
        .await
        .expect("the manifest must be derivable from the workspace");
    let fingerprint = manifest.fingerprint();
    ManifestStore::new(repo_root.to_path_buf())
        .record_terminal(
            &manifest,
            ManifestTerminal {
                hold_category: Some(AcceptanceHoldCategory::ReviewDeadlineExhausted),
                fingerprint: fingerprint.clone(),
                cleanup_confirmed: true,
                cleanup_diagnostics: "owned process group confirmed quiescent".to_string(),
                recorded_at: chrono::Utc::now(),
            },
        )
        .expect("the terminal manifest must persist");
    fingerprint
}

/// An unchanged fingerprint dispatches nothing: no gate, no reviewer. The
/// refusal is re-derived from the worktree, so it survives a restart that
/// discarded every in-memory hold.
#[tokio::test]
async fn acceptance_execution_boundary_unchanged_input_dispatches_nothing() {
    let repo_root = workspace_with(&proposal_with_gate("sh -c true")).await;
    let config = config_with_reviewer(canonical_reviewer());
    let fingerprint = record_hold(repo_root.path(), &config).await;

    let (result, iteration) = run_acceptance(repo_root.path(), &config, &runner(3600)).await;

    match result {
        AcceptanceResult::ExecutionHold { hold } => {
            assert_eq!(hold.fingerprint, fingerprint);
            assert_eq!(
                hold.category,
                AcceptanceHoldCategory::ReviewDeadlineExhausted
            );
            assert!(hold.summary(CHANGE).contains("unchanged_acceptance_input"));
        }
        other => panic!("unchanged input must be refused, got {other:?}"),
    }
    assert_eq!(iteration, 0);
    assert!(
        !reviewer_started(repo_root.path()),
        "no reviewer may be dispatched for an unchanged input"
    );
    assert!(
        !repo_root
            .path()
            .join(".cflx/verification-evidence/focused-gate.json")
            .exists(),
        "no gate may be executed for an unchanged input"
    );
}

/// A repository-visible change to the candidate produces a new fingerprint and
/// permits exactly one fresh bounded attempt.
#[tokio::test]
async fn acceptance_execution_boundary_changed_input_permits_a_new_attempt() {
    let repo_root = workspace_with(&proposal_with_gate("sh -c true")).await;
    let config = config_with_reviewer(canonical_reviewer());
    let refused_fingerprint = record_hold(repo_root.path(), &config).await;

    // The sanctioned recovery: a real repository-visible change.
    std::fs::write(repo_root.path().join("repair.rs"), "fn repaired() {}\n")
        .or_fail("unexpected error");
    commit_all(repo_root.path(), "Apply: repair").await;

    let (result, _) = run_acceptance(repo_root.path(), &config, &runner(3600)).await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "a changed candidate must permit a new bounded attempt, got {result:?}"
    );
    assert!(reviewer_started(repo_root.path()));
    let stored = ManifestStore::new(repo_root.path().to_path_buf())
        .load(CHANGE)
        .expect("the new attempt rewrites the manifest");
    assert_ne!(stored.fingerprint(), refused_fingerprint);
    assert!(
        stored.terminal.is_none(),
        "a fresh attempt must not inherit the previous terminal record"
    );
}

/// Editing the change's own artifacts is equally sufficient recovery: the
/// refusal names `change_inputs`, so that has to work.
#[tokio::test]
async fn acceptance_execution_boundary_changed_change_inputs_permit_a_new_attempt() {
    let repo_root = workspace_with(PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(canonical_reviewer());
    record_hold(repo_root.path(), &config).await;

    std::fs::write(
        repo_root
            .path()
            .join("openspec/changes")
            .join(CHANGE)
            .join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n- [x] 2. also done\n",
    )
    .or_fail("unexpected error");

    let (result, _) = run_acceptance(repo_root.path(), &config, &runner(3600)).await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "changed change inputs must restore eligibility, got {result:?}"
    );
}

/// Restart: a real deadline hold, then an entirely fresh set of runtime objects
/// — new agent, new history, new runner, no carried-over value of any kind —
/// reaches the same refusal from the preserved worktree alone.
///
/// This is constitutional law 1 end to end. The first invocation's hold lives
/// only in the worktree; everything else about it is discarded before the
/// second invocation runs, and the answer does not change.
#[tokio::test]
async fn acceptance_execution_boundary_restart_reaches_the_same_refusal() {
    let repo_root = workspace_with(PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(non_terminating_reviewer());

    let (first, _) = scoped_review_budget_millis_for_test(
        150,
        run_acceptance(repo_root.path(), &config, &runner(3600)),
    )
    .await;
    assert!(first.is_execution_hold(), "{first:?}");
    std::fs::remove_file(repo_root.path().join(REVIEWER_MARKER)).or_fail("unexpected error");

    // "Restart": every runtime object below is new, and the only thing that
    // survives is the worktree.
    let restarted_config = config_with_reviewer(canonical_reviewer());
    let (second, iteration) =
        run_acceptance(repo_root.path(), &restarted_config, &runner(3600)).await;

    match second {
        AcceptanceResult::ExecutionHold { hold } => {
            assert_eq!(
                hold.category,
                AcceptanceHoldCategory::ReviewDeadlineExhausted
            );
        }
        other => panic!("a restart must re-derive the refusal, got {other:?}"),
    }
    assert_eq!(iteration, 0);
    assert!(
        !reviewer_started(repo_root.path()),
        "the restarted process must dispatch no reviewer for an unchanged input"
    );
}

/// A hold recorded against a *different* fingerprint never refuses the current
/// one. The guard stops one specific loop; it is not a general gate.
#[tokio::test]
async fn acceptance_execution_boundary_foreign_fingerprint_does_not_refuse() {
    let repo_root = workspace_with(PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(canonical_reviewer());
    let manifest =
        crate::orchestration::acceptance::manifest_builder::build_manifest_for_workspace(
            &crate::orchestration::acceptance::verification_evidence::GitRepositoryFacts,
            repo_root.path(),
            CHANGE,
            config.get_accept_skill(),
            Some("main"),
            3600,
        )
        .await
        .expect("unexpected error");
    ManifestStore::new(repo_root.path().to_path_buf())
        .record_terminal(
            &manifest,
            ManifestTerminal {
                hold_category: Some(AcceptanceHoldCategory::ReviewDeadlineExhausted),
                fingerprint: "f".repeat(64),
                cleanup_confirmed: true,
                cleanup_diagnostics: String::new(),
                recorded_at: chrono::Utc::now(),
            },
        )
        .or_fail("unexpected error");

    let (result, _) = run_acceptance(repo_root.path(), &config, &runner(3600)).await;

    assert!(matches!(result, AcceptanceResult::Pass), "{result:?}");
}
