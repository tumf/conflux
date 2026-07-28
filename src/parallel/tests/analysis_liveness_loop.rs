//! Full scheduler-loop liveness regressions for unusable dependency-analysis results.
//!
//! `unchanged_analysis_input.rs` drives one loop step at a time, which is the right shape for
//! probe counting and suppression timing but cannot observe loop *termination*: it calls the
//! next iteration itself, so a pass that asked the scheduler to stop still looks alive.
//!
//! These tests therefore run the real
//! [`ParallelExecutor::execute_with_order_based_reanalysis`] loop under paused Tokio time. The
//! analyzer double decides when the run ends by cancelling the scheduler, so a loop that exits
//! on its own is distinguishable from one that stayed alive: a self-exit reports
//! [`ParallelEvent::AllCompleted`] with too few analyses, while a live loop reports
//! [`ParallelEvent::Stopped`] after the expected retry.

use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::openspec::{Change, ProposalMetadata};
use crate::parallel::{ParallelEvent, ParallelExecutor};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// The existing queue-coalescing debounce window, which also bounds fail-open retry cadence.
const QUEUE_DEBOUNCE: Duration = Duration::from_secs(10);

/// The scheduler's ordinary timer branch duration.
const SCHEDULER_TIMER: Duration = Duration::from_millis(500);

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

/// What the real scheduler loop did during one run.
struct LoopObservation {
    analyses: usize,
    /// Paused-time instants at which each analysis started, relative to the run start.
    analysis_offsets: Vec<Duration>,
    all_completed: bool,
    stopped: bool,
}

/// What the analyzer double should do on each invocation.
struct LoopAnalyzerScript {
    order: Vec<String>,
    stop_after_analyses: usize,
    started_at: tokio::time::Instant,
    invocations: Arc<AtomicUsize>,
    offsets: Arc<StdMutex<Vec<Duration>>>,
    cancel_token: CancellationToken,
}

/// Analyzer double that records when each analysis started and cancels the scheduler once the
/// behaviour under test has been observed.
fn loop_analyzer(
    script: Arc<LoopAnalyzerScript>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |_changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        let started = script.invocations.fetch_add(1, Ordering::SeqCst) + 1;
        script
            .offsets
            .lock()
            .expect("offsets lock")
            .push(script.started_at.elapsed());
        if started >= script.stop_after_analyses {
            script.cancel_token.cancel();
        }
        let order = script.order.clone();
        Box::pin(async move {
            AnalysisOutcome::new(
                AnalysisResult {
                    order,
                    dependencies: std::collections::HashMap::new(),
                    groups: None,
                },
                AnalysisProvenance::HealthyLlm,
            )
        })
    }
}

/// Run the real scheduler loop with an analyzer that returns `order` and cancels the scheduler
/// once `stop_after_analyses` analyses have started.
///
/// Cancelling from inside the analyzer is what makes the test finite without weakening it: the
/// loop is never asked to stop until the behaviour under test has already been observed.
async fn run_scheduler_loop(
    repo_root: &std::path::Path,
    queued: Vec<Change>,
    order: Vec<String>,
    stop_after_analyses: usize,
) -> LoopObservation {
    let workspace_base = TempDir::new().expect("create workspace base");
    let (event_tx, mut events) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(
        repo_root.to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    let stall_state_root = TempDir::new().expect("create stall state root");
    executor.set_acceptance_stall_state_root(stall_state_root.path().to_path_buf());
    let cancel_token = CancellationToken::new();
    executor.set_cancel_token(cancel_token.clone());

    let script = Arc::new(LoopAnalyzerScript {
        order,
        stop_after_analyses,
        started_at: tokio::time::Instant::now(),
        invocations: Arc::new(AtomicUsize::new(0)),
        offsets: Arc::new(StdMutex::new(Vec::new())),
        cancel_token,
    });
    let analyzer = loop_analyzer(script.clone());

    executor
        .execute_with_order_based_reanalysis(queued, analyzer)
        .await
        .expect("scheduler loop must not fail");

    let (mut all_completed, mut stopped) = (false, false);
    while let Ok(event) = events.try_recv() {
        match event {
            ParallelEvent::AllCompleted => all_completed = true,
            ParallelEvent::Stopped => stopped = true,
            _ => {}
        }
    }

    let analysis_offsets = script.offsets.lock().expect("offsets lock").clone();
    LoopObservation {
        analyses: script.invocations.load(Ordering::SeqCst),
        analysis_offsets,
        all_completed,
        stopped,
    }
}

#[tokio::test(start_paused = true)]
async fn empty_analysis_order_keeps_the_queued_scheduler_loop_alive_and_retries() {
    let repo_dir = TempDir::new().unwrap();
    init_minimal_git_repo(repo_dir.path());

    // Queued work remains and nothing is in flight — exactly the state in which an empty order
    // used to be read as a fully drained scheduler.
    let observation = run_scheduler_loop(
        repo_dir.path(),
        vec![test_change("queued-a")],
        Vec::new(),
        2,
    )
    .await;

    assert!(
        !observation.all_completed,
        "an unusable empty order must not terminate the loop while queued work remains"
    );
    assert!(
        observation.stopped,
        "the loop must have still been running when the test cancelled it"
    );
    assert_eq!(
        observation.analyses, 2,
        "the loop must survive the first unusable result and retry once"
    );

    let retry_delay = observation.analysis_offsets[1] - observation.analysis_offsets[0];
    assert!(
        retry_delay >= QUEUE_DEBOUNCE,
        "the retry must wait for the next debounce-eligible evaluation, not the next 500 ms \
         wake; retried after {retry_delay:?}"
    );
    assert!(
        retry_delay < QUEUE_DEBOUNCE + SCHEDULER_TIMER * 4,
        "the retry must still happen promptly at the bounded cadence; retried after {retry_delay:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn fully_drained_scheduler_still_terminates() {
    let repo_dir = TempDir::new().unwrap();
    init_minimal_git_repo(repo_dir.path());

    // No queued work and nothing in flight: the canonical drain check still owns termination,
    // and it must not be weakened by the unusable-result change.
    let observation = run_scheduler_loop(repo_dir.path(), Vec::new(), Vec::new(), 1).await;

    assert!(
        observation.all_completed,
        "a fully drained finite scheduler must still terminate on its own"
    );
    assert!(
        !observation.stopped,
        "a drained scheduler must exit through completion rather than cancellation"
    );
    assert_eq!(
        observation.analyses, 0,
        "a drained scheduler must not invoke the analyzer"
    );
}
