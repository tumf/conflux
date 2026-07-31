//! Cancellation-exit regressions for the real parallel scheduler loop.
//!
//! Truthful idle-vs-force stop classification depends on what the scheduler leaves behind
//! when an operator cancels a run. Two scheduler-owned obligations must therefore complete
//! *before* [`ParallelEvent::Stopped`] establishes terminal stop:
//!
//! - Registered per-change execution handles must be released. Cancellation aborts
//!   in-flight workspace tasks, so they never reach `handle_workspace_completion`; the
//!   [`DynamicQueue`] outlives one run (one per TUI session), so a surviving handle would
//!   later be read as positive execution evidence and turn an idle second Esc into a
//!   "Force stopped" claim about a process that no longer exists.
//! - Pending background merge / base-lane results must be handled. Those tasks are
//!   detached, so cancellation does not abort them; announcing "Processing stopped" while
//!   one is still mutating the base repository is untruthful.
//!
//! Both tests drive the real
//! [`ParallelExecutor::execute_with_order_based_reanalysis`] loop and assert on its event
//! stream, so each assertion holds at the moment `Stopped` is emitted rather than merely at
//! some point after the run returned. The analyzer double cancels the run from inside the
//! first analysis, and an empty analysis order keeps the loop from dispatching real
//! workspace work; the pending background merge is a channel double, so no detached merge
//! task is spawned.
//!
//! Tokio time is deliberately not paused here: the loop shells out to git while preparing
//! for parallel execution, and paused-time auto-advance fires test timers instead of waiting
//! for those child processes. The waits below are therefore real but short, keeping both
//! tests inside the 1s default-suite limit.

use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::openspec::{Change, ProposalMetadata};
use crate::parallel::{
    MergeResult, MergeResultOrigin, MergeTaskOutcome, ParallelEvent, ParallelExecutor,
};
use crate::tui::queue::DynamicQueue;
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Bounded wait for an event that must arrive; only reached when the assertion fails.
const EVENT_WAIT: Duration = Duration::from_millis(500);

/// Window in which terminal stop must NOT be observed while shutdown work is outstanding.
const NO_STOP_WINDOW: Duration = Duration::from_millis(150);

type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

fn test_config(workspace_base: &std::path::Path) -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        analyze_command: Some("echo analyze".to_string()),
        acceptance_command: Some("echo acceptance".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        ..Default::default()
    }
}

fn test_change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: String::new(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

fn init_minimal_git_repo(repo_root: &std::path::Path) {
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test User"],
    ] {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git setup command");
        assert!(output.status.success(), "git setup command failed");
    }
    std::fs::write(repo_root.join("README.md"), "base\n").expect("write base file");
    for args in [vec!["add", "-A"], vec!["commit", "-m", "Base"]] {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git commit command");
        assert!(output.status.success(), "git commit command failed");
    }
}

/// Analyzer double that returns an empty order (so nothing is dispatched) and cancels the
/// run from inside its first invocation.
///
/// Cancelling from the analyzer keeps the test deterministic: the loop is only asked to stop
/// once it is genuinely running, and it always observes the cancellation at the next loop
/// iteration boundary, which is the exit under test.
fn cancelling_analyzer(
    cancel_token: CancellationToken,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |_changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        cancel_token.cancel();
        Box::pin(async move {
            AnalysisOutcome::new(
                AnalysisResult {
                    order: Vec::new(),
                    dependencies: std::collections::HashMap::new(),
                    groups: None,
                },
                AnalysisProvenance::HealthyLlm,
            )
        })
    }
}

/// Receive scheduler events until `Stopped`, collecting the log messages seen before it.
///
/// Logs are accumulated into the caller's buffer so a bounded wait that is *expected* to time
/// out (terminal stop must not happen yet) still leaves its observations for later assertions.
async fn recv_until_stopped(events: &mut mpsc::Receiver<ParallelEvent>, logs: &mut Vec<String>) {
    while let Some(event) = events.recv().await {
        match event {
            ParallelEvent::Log(entry) => logs.push(entry.message),
            ParallelEvent::Stopped => return,
            ParallelEvent::AllCompleted => {
                panic!("a cancelled run must not report AllCompleted; logs: {logs:?}")
            }
            _ => {}
        }
    }
    panic!("scheduler event stream closed before ParallelEvent::Stopped; logs: {logs:?}");
}

#[tokio::test]
async fn idle_parallel_stop_cancelled_scheduler_releases_registered_execution_handles() {
    let repo_dir = TempDir::new().unwrap();
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("create workspace base");
    let stall_state_root = TempDir::new().expect("create stall state root");

    let (event_tx, mut events) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    executor.set_acceptance_stall_state_root(stall_state_root.path().to_path_buf());
    let cancel_token = CancellationToken::new();
    executor.set_cancel_token(cancel_token.clone());

    let queue = Arc::new(DynamicQueue::new());
    executor.set_dynamic_queue(queue.clone());

    // An in-flight change owns a registered execution handle, exactly as
    // `dispatch_change_to_workspace` registers one for every dispatched workspace task.
    // Cancellation aborts such a task, so nothing else will release this handle.
    queue
        .register_kill_token("in-flight-change".to_string(), CancellationToken::new())
        .await;
    let done = queue
        .request_cancellation("in-flight-change")
        .await
        .expect("the in-flight change must own a registered execution handle");
    assert_eq!(
        queue.registered_execution_count().await,
        1,
        "test setup must leave one registered execution handle"
    );

    let analyzer = cancelling_analyzer(cancel_token);
    let scheduler = tokio::spawn(async move {
        executor
            .execute_with_order_based_reanalysis(vec![test_change("queued-a")], analyzer)
            .await
    });

    let mut logs = Vec::new();
    tokio::time::timeout(EVENT_WAIT, recv_until_stopped(&mut events, &mut logs))
        .await
        .expect("cancelled scheduler must reach terminal stop");

    assert_eq!(
        queue.registered_execution_count().await,
        0,
        "registered execution handles must be released before terminal stop, otherwise a later \
         idle stop in the same session is misclassified as a force stop; logs: {logs:?}"
    );
    assert!(
        done.is_cancelled(),
        "releasing a handle must fire its done handshake so waiters learn the task ended"
    );

    scheduler
        .await
        .expect("scheduler task must not panic")
        .expect("cancelled scheduler loop must not fail");
}

#[tokio::test]
async fn idle_parallel_stop_cancelled_scheduler_handles_pending_merge_before_stopped() {
    let repo_dir = TempDir::new().unwrap();
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("create workspace base");
    let stall_state_root = TempDir::new().expect("create stall state root");

    let (event_tx, mut events) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    executor.set_acceptance_stall_state_root(stall_state_root.path().to_path_buf());
    let cancel_token = CancellationToken::new();
    executor.set_cancel_token(cancel_token.clone());

    // Channel double for the loop's background merge/base-lane results, plus an outstanding
    // pending merge: this is the state a detached post-archive merge task leaves behind when
    // the operator cancels before it reports.
    let (merge_result_tx, merge_result_rx) = mpsc::channel(8);
    let merge_task = merge_result_tx.clone();
    executor.merge_result_channel_override = Some((merge_result_tx, merge_result_rx));
    let pending_merge_count = executor.pending_merge_count.clone();
    pending_merge_count.store(1, Ordering::SeqCst);

    let analyzer = cancelling_analyzer(cancel_token);
    let mut scheduler = tokio::spawn(async move {
        executor
            .execute_with_order_based_reanalysis(vec![test_change("queued-a")], analyzer)
            .await
    });

    // Terminal stop must not be established while the merge is still in flight.
    let mut logs = Vec::new();
    let premature =
        tokio::time::timeout(NO_STOP_WINDOW, recv_until_stopped(&mut events, &mut logs)).await;
    assert!(
        premature.is_err(),
        "the cancelled scheduler must not emit Stopped while a background merge is pending; \
         logs: {logs:?}"
    );
    assert_eq!(
        pending_merge_count.load(Ordering::SeqCst),
        1,
        "the pending merge must still be outstanding while the scheduler waits"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(1), &mut scheduler)
            .await
            .is_err(),
        "the scheduler loop must still be inside its cleanup barrier"
    );

    // The detached merge task finally reports its outcome.
    merge_task
        .send(MergeResult {
            change_id: "merging-change".to_string(),
            workspace_name: "merging-change".to_string(),
            origin: MergeResultOrigin::PostArchiveMerge,
            outcome: Ok(MergeTaskOutcome::deferred("base lane busy", false)),
        })
        .await
        .expect("scheduler must still hold the merge-result receiver");

    tokio::time::timeout(EVENT_WAIT, recv_until_stopped(&mut events, &mut logs))
        .await
        .expect("the scheduler must stop once the pending merge reported");

    assert_eq!(
        pending_merge_count.load(Ordering::SeqCst),
        0,
        "the pending merge result must be handled before terminal stop; logs: {logs:?}"
    );
    assert!(
        logs.iter()
            .any(|message| message.contains("pending background merge/base-lane task")),
        "the operator must see why stopping waits; logs: {logs:?}"
    );

    scheduler
        .await
        .expect("scheduler task must not panic")
        .expect("cancelled scheduler loop must not fail");
}
