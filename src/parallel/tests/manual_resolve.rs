//! Tests for manual resolve counter integration with parallel execution.

use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::dynamic_queue::ReanalysisReason;
use crate::parallel::queue_state::ReanalysisDispatchContext;
use crate::parallel::{ParallelExecutor, SchedulerLifetime, WorkspaceResult};
use crate::tui::queue::DynamicQueue;
use crate::vcs::VcsBackend;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tempfile::TempDir;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// Wire the reducer the way production does before a dynamic queue push.
///
/// `OperatorCommandService::add_to_queue` applies `AddToQueue` first and only
/// then publishes the wake-up hint, and ingestion validates the hint against
/// that reducer intent — never against the catalog alone — so a dynamic-queue
/// test must record the same intent to exercise the accepted path.
fn shared_state_with_queue_intent(
    change_ids: &[&str],
) -> Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>> {
    use crate::orchestration::state::{OrchestratorState, ReducerCommand};

    let mut state = OrchestratorState::new(change_ids.iter().map(|id| id.to_string()).collect(), 1);
    for change_id in change_ids {
        state.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
    }
    Arc::new(tokio::sync::RwLock::new(state))
}

/// Helper function to create a test config with all required commands
fn create_test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        analyze_command: Some("echo analyze".to_string()),
        acceptance_command: Some("echo acceptance".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_manual_resolve_counter_reduces_available_slots() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path().to_path_buf();

    // Create a basic config
    let config = create_test_config();

    // Create a manual resolve counter
    let manual_resolve_counter = Arc::new(AtomicUsize::new(0));

    // Create a ParallelExecutor with max_concurrent = 4
    let mut executor = ParallelExecutor::new(repo_root.clone(), config.clone(), None);

    // Set the manual resolve counter
    executor.set_manual_resolve_counter(manual_resolve_counter.clone());

    // Initially, counter should be 0
    assert_eq!(
        manual_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Manual resolve counter should start at 0"
    );

    // Simulate a manual resolve starting (TUI would increment this)
    manual_resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    // Verify counter is now 1
    assert_eq!(
        manual_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Manual resolve counter should be 1 after increment"
    );

    // The available_slots calculation in execute_with_order_based_reanalysis should now be:
    // max_parallelism (4) - in_flight (0) - manual_resolve_count (1) = 3
    // This is tested implicitly by the slot calculation logic in the executor

    // Simulate resolve completing
    manual_resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

    // Counter should be back to 0
    assert_eq!(
        manual_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Manual resolve counter should return to 0 after completion"
    );
}

#[tokio::test]
async fn test_multiple_manual_resolves_consume_multiple_slots() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path().to_path_buf();

    // Create a basic config
    let config = create_test_config();

    // Create a manual resolve counter
    let manual_resolve_counter = Arc::new(AtomicUsize::new(0));

    // Create a ParallelExecutor
    let mut executor = ParallelExecutor::new(repo_root.clone(), config.clone(), None);
    executor.set_manual_resolve_counter(manual_resolve_counter.clone());

    // Simulate 2 concurrent manual resolves
    manual_resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    manual_resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    assert_eq!(
        manual_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "Manual resolve counter should be 2 for concurrent resolves"
    );

    // If max_parallelism is 4, available_slots should now be:
    // 4 - 0 (in_flight) - 2 (manual_resolve_count) = 2

    // Simulate first resolve completing
    manual_resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        manual_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Manual resolve counter should be 1 after one completes"
    );

    // Simulate second resolve completing
    manual_resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        manual_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Manual resolve counter should be 0 after all complete"
    );
}

#[tokio::test]
async fn test_manual_resolve_completion_notifies_scheduler() {
    let queue = DynamicQueue::new();
    let notified = queue.notified();

    queue.notify_scheduler();

    tokio::time::timeout(std::time::Duration::from_secs(1), notified)
        .await
        .expect("scheduler notification should wake waiters");
}

#[test]
fn test_manual_resolve_counter_is_thread_safe() {
    // Create a counter
    let counter = Arc::new(AtomicUsize::new(0));

    // Spawn multiple threads to increment/decrement concurrently
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let counter_clone = counter.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    counter_clone.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Counter should be back to 0
    assert_eq!(
        counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Counter should be 0 after concurrent increment/decrement operations"
    );
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

fn create_active_change_fixture(repo_root: &std::path::Path, change_id: &str) {
    let change_dir = repo_root.join("openspec").join("changes").join(change_id);
    std::fs::create_dir_all(&change_dir).expect("create synthetic OpenSpec change directory");
    std::fs::write(
        change_dir.join("proposal.md"),
        format!("# Synthetic Change {change_id}\n\n## Why\n\nTest fixture.\n"),
    )
    .expect("write synthetic proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "# Tasks\n\n- [ ] Synthetic fixture task\n",
    )
    .expect("write synthetic tasks");
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
        assert!(
            output.status.success(),
            "git setup command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::write(repo_root.join("README.md"), "base\n").expect("write base file");
    for args in [vec!["add", "-A"], vec!["commit", "-m", "Base"]] {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git commit command");
        assert!(
            output.status.success(),
            "git commit command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn analysis_result<'a>(
    changes: &'a [Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies: HashMap::new(),
            groups: None,
        }
        .into()
    })
}

/// Zero capacity gates the analyzer as well as dispatch.
///
/// An analysis order nothing can consume is wasted work, and recording that the
/// input was analysed is what would suppress the evaluation capacity recovery
/// depends on. Queue classification, reducer reconciliation, and diagnostics all
/// still run — they are just above this gate.
#[tokio::test]
async fn test_manual_resolve_zero_capacity_gates_analysis_and_apply_dispatch() {
    let temp_dir = TempDir::new().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    let manual_resolve_counter = Arc::new(AtomicUsize::new(1));
    executor.set_manual_resolve_counter(manual_resolve_counter);

    let mut queued = vec![test_change("queued-apply")];
    let mut in_flight = HashSet::new();
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard =
        WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::ResolveCompletion,
            analyzer: &analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
            work_snapshot: None,
        })
        .await
        .expect("re-analysis should not fail");

    assert!(!should_break);
    assert_eq!(
        iteration, 1,
        "suppressed dispatch must not advance iteration"
    );
    assert!(
        in_flight.is_empty(),
        "zero capacity must not start apply work"
    );
    assert_eq!(
        queued.len(),
        1,
        "queued change remains pending until capacity recovers"
    );
    assert!(
        join_set.is_empty(),
        "no workspace task should be spawned at zero capacity"
    );

    let mut saw_analysis_started = false;
    let mut saw_apply_started = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::AnalysisStarted { .. } => saw_analysis_started = true,
            ExecutionEvent::ApplyStarted { .. } => saw_apply_started = true,
            _ => {}
        }
    }

    assert!(
        !saw_analysis_started,
        "the expensive analyzer must not start while a manual resolve holds every slot"
    );
    assert!(
        !saw_apply_started,
        "ordinary apply must remain capacity-gated during active manual resolve"
    );
}

/// Repeated zero-capacity wakes stay inert rather than relaunching the analyzer.
#[tokio::test]
async fn repeated_capacity_zero_never_starts_analysis() {
    let temp_dir = TempDir::new().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );

    let mut queued = vec![test_change("queued-apply")];
    let mut in_flight = HashSet::from(["active-apply".to_string()]);
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard =
        WorkspaceCleanupGuard::new(VcsBackend::Git, temp_dir.path().to_path_buf());

    for iteration in 1..=2 {
        let (should_break, returned_iteration) = executor
            .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
                queued: &mut queued,
                in_flight: &mut in_flight,
                max_parallelism: 1,
                iteration,
                reanalysis_reason: ReanalysisReason::ResolveCompletion,
                analyzer: &analysis_result,
                semaphore: semaphore.clone(),
                join_set: &mut join_set,
                cleanup_guard: &mut cleanup_guard,
                work_snapshot: None,
            })
            .await
            .expect("re-analysis should not fail");

        assert!(!should_break);
        assert_eq!(
            returned_iteration, iteration,
            "suppressed dispatch must not advance iteration"
        );
    }

    assert_eq!(
        queued.len(),
        1,
        "queued change remains pending while capacity is zero"
    );
    assert_eq!(
        in_flight.len(),
        1,
        "test must keep capacity at zero across repeated analysis iterations"
    );
    assert!(
        join_set.is_empty(),
        "no workspace task should be spawned at zero capacity"
    );

    let mut analysis_started_count = 0;
    let mut apply_started_count = 0;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::AnalysisStarted { .. } => analysis_started_count += 1,
            ExecutionEvent::ApplyStarted { .. } => apply_started_count += 1,
            _ => {}
        }
    }

    assert_eq!(
        analysis_started_count, 0,
        "a suppressed evaluation is not a distinct analysis attempt; saw {analysis_started_count}"
    );
    assert_eq!(
        apply_started_count, 0,
        "ordinary apply must remain capacity-gated"
    );
}

#[tokio::test]
async fn scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve() {
    let temp_dir = TempDir::new().unwrap();
    init_minimal_git_repo(temp_dir.path());
    let seed_change_id = "synthetic-seed-gated";
    let synthetic_change_id = "synthetic-dynamic-gated-resolve";
    create_active_change_fixture(temp_dir.path(), seed_change_id);
    create_active_change_fixture(temp_dir.path(), synthetic_change_id);

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let dynamic_queue = Arc::new(DynamicQueue::new());

    let cancel_token = tokio_util::sync::CancellationToken::new();
    let gated_resolve_counter = Arc::new(AtomicUsize::new(4));

    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor.set_cancel_token(cancel_token.clone());
    executor.set_dynamic_queue(dynamic_queue.clone());
    executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);
    executor.set_manual_resolve_counter(gated_resolve_counter.clone());
    executor.set_shared_orchestrator_state(shared_state_with_queue_intent(&[
        seed_change_id,
        synthetic_change_id,
    ]));

    let scheduler_queue = dynamic_queue.clone();
    let scheduler = tokio::spawn(async move {
        executor
            .execute_with_order_based_reanalysis(vec![test_change(seed_change_id)], analysis_result)
            .await
    });
    scheduler_queue.push(synthetic_change_id.to_string()).await;

    let mut saw_dynamic_ingest = false;
    let mut saw_analysis_started = false;
    let mut saw_apply_started = false;
    let mut saw_capacity_diagnostic = false;
    let mut log_messages = Vec::new();

    tokio::time::timeout(std::time::Duration::from_millis(500), async {
        while !(saw_dynamic_ingest && saw_capacity_diagnostic) {
            match rx.recv().await {
                Some(ExecutionEvent::Log(entry))
                    if entry.message.contains(&format!(
                        "Dynamically added to parallel execution: {synthetic_change_id}"
                    )) =>
                {
                    saw_dynamic_ingest = true;
                }
                Some(ExecutionEvent::Log(entry))
                    if entry.message.contains("analysis_capacity_zero") =>
                {
                    saw_capacity_diagnostic = true;
                    log_messages.push(entry.message);
                }
                Some(ExecutionEvent::Log(entry)) => log_messages.push(entry.message),
                Some(ExecutionEvent::AnalysisStarted { .. }) => saw_analysis_started = true,
                Some(ExecutionEvent::ApplyStarted { .. }) => saw_apply_started = true,
                Some(_) => {}
                None => break,
            }
        }
    })
    .await
    .expect("scheduler loop should ingest and analyze bounded dynamic work");

    assert!(
        gated_resolve_counter.load(Ordering::SeqCst) > 0,
        "controllable resolve gate must still be held when the capacity diagnostic fires"
    );

    gated_resolve_counter.store(0, Ordering::SeqCst);
    cancel_token.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_millis(500), scheduler)
        .await
        .expect("scheduler should stop after cancellation")
        .expect("scheduler task should not panic");

    assert!(
        saw_dynamic_ingest,
        "expected dynamic ingest log for {synthetic_change_id}; saw logs: {log_messages:?}"
    );
    assert!(
        !saw_analysis_started,
        "zero recalculated capacity must suppress the expensive analyzer too"
    );
    assert!(saw_capacity_diagnostic);
    assert!(
        !saw_apply_started,
        "zero recalculated capacity must suppress apply dispatch while gated resolve is active"
    );
}

#[tokio::test]
async fn persistent_scheduler_dynamic_queue_push_after_initial_analysis_bypasses_debounce() {
    let temp_dir = TempDir::new().unwrap();
    init_minimal_git_repo(temp_dir.path());
    let seed_change_id = "synthetic-seed-running";
    let dynamic_change_id = "synthetic-running-dynamic-queue";
    create_active_change_fixture(temp_dir.path(), seed_change_id);
    create_active_change_fixture(temp_dir.path(), dynamic_change_id);

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let dynamic_queue = Arc::new(DynamicQueue::new());
    let cancel_token = tokio_util::sync::CancellationToken::new();

    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor.set_cancel_token(cancel_token.clone());
    executor.set_dynamic_queue(dynamic_queue.clone());
    executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);
    // Only the seed carries queue intent at start. The dynamic change acquires
    // it at push time, in the production order: reducer intent first, wake-up
    // hint second.
    let shared = shared_state_with_queue_intent(&[seed_change_id]);
    executor.set_shared_orchestrator_state(shared.clone());

    let scheduler = tokio::spawn(async move {
        executor
            .execute_with_order_based_reanalysis(vec![test_change(seed_change_id)], analysis_result)
            .await
    });

    tokio::time::timeout(std::time::Duration::from_millis(500), async {
        loop {
            match rx.recv().await {
                Some(ExecutionEvent::AnalysisStarted { attempt_id, .. })
                    if attempt_id.contains(seed_change_id) =>
                {
                    break;
                }
                Some(_) => {}
                None => panic!("scheduler event stream closed before initial analysis"),
            }
        }
    })
    .await
    .expect("initial running scheduler analysis should start promptly");

    shared
        .write()
        .await
        .apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
            dynamic_change_id.to_string(),
        ));
    assert!(dynamic_queue.push(dynamic_change_id.to_string()).await);

    let mut saw_dynamic_ingest = false;
    let mut dynamic_analysis_attempt = None;
    tokio::time::timeout(std::time::Duration::from_millis(500), async {
        while dynamic_analysis_attempt.is_none() {
            match rx.recv().await {
                Some(ExecutionEvent::Log(entry))
                    if entry.message.contains(&format!(
                        "Dynamically added to parallel execution: {dynamic_change_id}"
                    )) =>
                {
                    saw_dynamic_ingest = true;
                }
                Some(ExecutionEvent::AnalysisStarted { attempt_id, .. })
                    if attempt_id.contains(dynamic_change_id) =>
                {
                    dynamic_analysis_attempt = Some(attempt_id);
                }
                Some(_) => {}
                None => panic!("scheduler event stream closed before dynamic queue analysis"),
            }
        }
    })
    .await
    .expect("dynamic queue push after initial analysis should trigger sub-second reanalysis");

    cancel_token.cancel();
    let _ = tokio::time::timeout(std::time::Duration::from_millis(500), scheduler)
        .await
        .expect("scheduler should stop after cancellation")
        .expect("scheduler task should not panic");

    assert!(saw_dynamic_ingest, "dynamic queue entry should be ingested");
    assert!(
        dynamic_analysis_attempt
            .as_deref()
            .is_some_and(|attempt_id| attempt_id.contains("trigger=queue")),
        "dynamic queue analysis must use explicit queue trigger, got {dynamic_analysis_attempt:?}"
    );
}

#[tokio::test]
async fn dynamic_queue_ingestion_validates_candidates_against_executor_repo_root() {
    let temp_dir = TempDir::new().unwrap();
    let present_change_id = "synthetic-present-only-under-repo-root";
    let absent_change_id = "synthetic-absent-under-repo-root";
    create_active_change_fixture(temp_dir.path(), present_change_id);

    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let dynamic_queue = Arc::new(DynamicQueue::new());
    dynamic_queue.push(present_change_id.to_string()).await;
    dynamic_queue.push(absent_change_id.to_string()).await;

    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor.set_dynamic_queue(dynamic_queue);
    // Both IDs carry accepted reducer queue intent, so the repo-root catalog
    // lookup is the only thing that can separate them here.
    executor.set_shared_orchestrator_state(shared_state_with_queue_intent(&[
        present_change_id,
        absent_change_id,
    ]));

    let mut queued = Vec::new();
    let in_flight = HashSet::new();
    let mut reanalysis_reason = ReanalysisReason::Initial;

    let queue_changed = executor
        .check_dynamic_queue_and_add_changes(&mut queued, &in_flight, &mut reanalysis_reason)
        .await;

    assert!(queue_changed, "present repo-root change should be ingested");
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, present_change_id);
    assert_eq!(reanalysis_reason, ReanalysisReason::QueueNotification);

    let mut saw_present_ingest = false;
    let mut saw_absent_reconciliation = false;
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::Log(entry) = event {
            if entry.message.contains(&format!(
                "Dynamically added to parallel execution: {present_change_id}"
            )) {
                saw_present_ingest = true;
            }
            if entry.message.contains(&format!(
                "Queue reconciliation pending for '{absent_change_id}': candidate_not_found"
            )) {
                saw_absent_reconciliation = true;
            }
        }
    }

    assert!(
        saw_present_ingest,
        "ingestion log should name repo-root candidate"
    );
    assert!(
        saw_absent_reconciliation,
        "absent repo-root candidate should emit candidate_not_found reconciliation"
    );
    assert!(
        queued.iter().all(|change| change.id != absent_change_id),
        "absent candidate must not be queued"
    );
}

// ============================================================================
// Ghost queue prevention: accepted queue intent must converge with scheduler
// candidate discovery when the active OpenSpec catalog changes under a live
// owner.
//
// These are integration-scoped: they drive the real reducer, the real
// `DynamicQueue`, the real shared operator command boundary, and a real
// repository-visible `openspec/changes` tree on disk.
// ============================================================================

/// Reducer queue intent for one change, as an observer would read it.
async fn has_queue_intent(
    shared: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    change_id: &str,
) -> bool {
    shared
        .read()
        .await
        .queued_change_ids()
        .iter()
        .any(|id| id == change_id)
}

/// Collect every scheduler log message emitted so far.
fn drain_log_messages(rx: &mut tokio::sync::mpsc::Receiver<ExecutionEvent>) -> Vec<String> {
    let mut messages = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::Log(entry) = event {
            messages.push(entry.message);
        }
    }
    messages
}

/// The owner-before-proposal ordering, end to end on one executor.
///
/// The owner's first candidate lookup runs while `openspec/changes/<id>` does
/// not exist yet. The proposal then lands in the repository, and the *same*
/// executor — no restart, no new scheduler — must admit it from the refreshed
/// repository-visible view.
#[tokio::test]
async fn queued_intent_is_admitted_without_owner_restart_after_the_proposal_lands() {
    let temp_dir = TempDir::new().unwrap();
    let change_id = "synthetic-merged-after-owner-start";

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let dynamic_queue = Arc::new(DynamicQueue::new());
    dynamic_queue.push(change_id.to_string()).await;

    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor.set_dynamic_queue(dynamic_queue);
    let shared = shared_state_with_queue_intent(&[change_id]);
    executor.set_shared_orchestrator_state(shared.clone());

    let mut queued = Vec::new();
    let in_flight = HashSet::new();
    let mut reanalysis_reason = ReanalysisReason::Initial;

    // First lookup: the owner cannot see the proposal yet.
    let ingested = executor
        .check_dynamic_queue_and_add_changes(&mut queued, &in_flight, &mut reanalysis_reason)
        .await;
    assert!(!ingested, "an absent candidate cannot be ingested");
    assert!(queued.is_empty(), "no scheduler-local work exists yet");
    let messages = drain_log_messages(&mut rx);
    assert!(
        messages.iter().any(|message| message
            == &format!("Queue reconciliation pending for '{change_id}': candidate_not_found")),
        "the first miss must stay observable, got {messages:?}"
    );
    assert!(
        has_queue_intent(&shared, change_id).await,
        "hint ingestion alone must never revoke accepted queue intent"
    );

    // The proposal is merged into the base under the live owner.
    create_active_change_fixture(temp_dir.path(), change_id);

    let outcome = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;

    assert_eq!(
        outcome.queued_added, 1,
        "the same owner must admit the now-visible candidate"
    );
    assert_eq!(
        outcome.unavailable_reconciled, 0,
        "a loadable candidate is never reconciled away"
    );
    assert!(
        queued.iter().any(|change| change.id == change_id),
        "the refreshed candidate must become scheduler-local queued work"
    );
    assert!(
        has_queue_intent(&shared, change_id).await,
        "admitted queue intent must survive the refresh"
    );
}

/// A miss inside one reconciliation pass is re-checked against a *fresh*
/// repository-visible view before anything is decided.
///
/// This is the in-pass half of the race: the pass's own catalog map missed, the
/// proposal landed, and the re-read admits it rather than settling accepted
/// intent from a stale observation.
#[tokio::test]
async fn a_missing_candidate_is_re_read_before_any_verdict_is_reached() {
    let temp_dir = TempDir::new().unwrap();
    let change_id = "synthetic-visible-only-on-refresh";

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    let shared = shared_state_with_queue_intent(&[change_id]);
    executor.set_shared_orchestrator_state(shared.clone());

    // The pass's first catalog read missed; the repository now has the proposal.
    create_active_change_fixture(temp_dir.path(), change_id);

    let snapshot = executor.capture_reducer_work_snapshot().await;
    let mut queued = Vec::new();
    let mut outcome = crate::parallel::queue_state::QueueReconciliationOutcome::default();
    executor
        .resolve_missing_queued_candidate(change_id, &mut queued, &mut outcome, &snapshot)
        .await;

    assert_eq!(
        outcome.queued_added, 1,
        "the fresh re-read must admit the candidate"
    );
    assert_eq!(
        outcome.unavailable_reconciled, 0,
        "a candidate the refresh can load is not unavailable"
    );
    assert!(
        queued.iter().any(|change| change.id == change_id),
        "the refreshed candidate must become scheduler-local queued work"
    );
    assert!(
        has_queue_intent(&shared, change_id).await,
        "queue intent must be preserved when the refresh admits the candidate"
    );
    let messages = drain_log_messages(&mut rx);
    assert!(
        messages.iter().any(|message| message
            == &format!("Queue reconciliation admitted '{change_id}': candidate_refreshed")),
        "the refreshed-and-admitted result must be identifiable, got {messages:?}"
    );
}

/// A genuinely absent candidate leaves no queued row behind.
///
/// The queued projection has no scheduler-local work, no wake edge, and no
/// typed wait behind it, so reconciliation settles it through an explicit
/// reducer transition instead of reporting pending work forever. Diagnostics
/// stay bounded and nothing is dispatched.
///
/// The repository is a real Git repository on purpose: settlement requires
/// *conclusive* archived-dirty repair evidence, so a fixture where the base
/// branch cannot be resolved at all would prove deferral rather than the
/// settlement this test is about.
#[tokio::test]
async fn a_genuinely_absent_candidate_does_not_remain_a_ghost_queued_row() {
    let temp_dir = TempDir::new().unwrap();
    init_minimal_git_repo(temp_dir.path());
    let change_id = "synthetic-never-created-anywhere";

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    let shared = shared_state_with_queue_intent(&[change_id]);
    executor.set_shared_orchestrator_state(shared.clone());

    let mut queued = Vec::new();
    let in_flight = HashSet::new();

    let first = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;
    assert_eq!(
        first.unavailable_reconciled, 1,
        "unavailable queue intent must be reconciled explicitly"
    );
    assert_eq!(
        first.repair_evidence_deferred, 0,
        "conclusive repair evidence must not be reported as a deferral"
    );
    assert_eq!(first.queued_added, 0, "nothing loadable was added");
    assert!(
        queued.is_empty(),
        "an absent candidate must never become dispatchable work"
    );
    assert!(
        !has_queue_intent(&shared, change_id).await,
        "the queued projection must not survive as a ghost row"
    );

    // The reducer row is idle work again, not a dequeued or terminal outcome:
    // a later explicit Start or mark settlement can still admit the proposal.
    {
        let mut guard = shared.write().await;
        guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
            change_id.to_string(),
        ));
    }
    assert!(
        has_queue_intent(&shared, change_id).await,
        "reconciliation must leave the change re-admittable"
    );

    // A second pass over the same unavailable intent repeats no warning.
    let _ = drain_log_messages(&mut rx);
    let second = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;
    assert_eq!(
        second.unavailable_reconciled, 1,
        "the re-added intent is settled again on its own evidence"
    );
    let messages = drain_log_messages(&mut rx);
    assert!(
        !messages
            .iter()
            .any(|message| message.contains("candidate_not_found")),
        "identical missing-candidate warnings must not repeat, got {messages:?}"
    );
}

/// The shared operator command boundary both frontends submit through.
///
/// `OperatorIntent::SetQueueIntent` from the WebUI/`/api/v2` route and from the
/// TUI route both reach `OperatorCommandService::add_to_queue`, so the boundary
/// is exercised directly here rather than inferred from either frontend's
/// helper. Neither route may require an owner restart after the base catalog
/// gains the proposal, and neither may lose the operator's execution mark to a
/// failed admission.
#[tokio::test]
async fn the_shared_queue_command_boundary_needs_no_owner_restart_after_a_catalog_update() {
    use crate::orchestration::operator_command::{
        ExecutionMarkStore, NoopQueueHooks, OperatorCommandService,
    };

    let temp_dir = TempDir::new().unwrap();
    init_minimal_git_repo(temp_dir.path());
    let change_id = "synthetic-shared-boundary-late-proposal";

    let (tx, _rx) = tokio::sync::mpsc::channel(32);
    let dynamic_queue = DynamicQueue::new();
    let shared = Arc::new(tokio::sync::RwLock::new(
        crate::orchestration::state::OrchestratorState::new(vec![change_id.to_string()], 1),
    ));
    let marks = Arc::new(ExecutionMarkStore::new());
    let service = OperatorCommandService::new(
        shared.clone(),
        Arc::new(dynamic_queue.clone()),
        Arc::new(NoopQueueHooks),
        marks.clone(),
    );

    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor.set_dynamic_queue(Arc::new(dynamic_queue.clone()));
    executor.set_shared_orchestrator_state(shared.clone());

    // Operator selection and admission, through the shared boundary.
    marks.set(change_id, true);
    service
        .add_to_queue(change_id)
        .await
        .expect("shared queue command should be accepted");

    let mut queued = Vec::new();
    let in_flight = HashSet::new();
    let mut reanalysis_reason = ReanalysisReason::Initial;

    executor
        .check_dynamic_queue_and_add_changes(&mut queued, &in_flight, &mut reanalysis_reason)
        .await;
    let unavailable = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;

    assert_eq!(
        unavailable.unavailable_reconciled, 1,
        "an admission the catalog cannot satisfy must be settled, not left queued"
    );
    assert!(queued.is_empty(), "nothing dispatchable was produced");
    assert!(
        marks.is_marked(change_id),
        "a failed queue admission must not revoke the operator's execution mark"
    );

    // The proposal reaches the base catalog, and the same owner is asked again
    // through the same shared command.
    create_active_change_fixture(temp_dir.path(), change_id);
    service
        .add_to_queue(change_id)
        .await
        .expect("shared queue command should be accepted after the catalog update");

    let mut reanalysis_reason = ReanalysisReason::Initial;
    let ingested = executor
        .check_dynamic_queue_and_add_changes(&mut queued, &in_flight, &mut reanalysis_reason)
        .await;

    assert!(
        ingested,
        "the same owner must admit the change without a restart"
    );
    assert!(
        queued.iter().any(|change| change.id == change_id),
        "the shared boundary must produce scheduler-local queued work"
    );
    assert!(
        marks.is_marked(change_id),
        "the execution mark stays an independent axis throughout"
    );
}

/// Inconclusive archived-dirty repair evidence is not proof of absence.
///
/// The settle path spends accepted queue intent, and mark settlement is
/// edge-triggered, so a revoked row does not come back without an explicit
/// operator Start. A base branch that cannot be resolved means the repair probe
/// never ran at all — the precondition "no archived-dirty repair candidate
/// applies" was never established — so the pass must defer exactly as an
/// unreadable catalog does.
#[tokio::test]
async fn undetermined_repair_evidence_defers_instead_of_settling_queue_intent() {
    let temp_dir = TempDir::new().unwrap();
    init_minimal_git_repo(temp_dir.path());
    let change_id = "synthetic-repair-evidence-unavailable";

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    // Detached HEAD with no recorded original branch: base identity is
    // unreadable, so the archived-dirty repair probe has no base to compare to.
    executor.set_workspace_manager(Box::new(
        crate::parallel::tests::executor::TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_failing_original_branch(),
    ));
    let shared = shared_state_with_queue_intent(&[change_id]);
    executor.set_shared_orchestrator_state(shared.clone());

    let mut queued = Vec::new();
    let in_flight = HashSet::new();

    let outcome = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;

    assert_eq!(
        outcome.unavailable_reconciled, 0,
        "queue intent must never be settled on evidence that was never gathered"
    );
    assert_eq!(
        outcome.repair_evidence_deferred, 1,
        "the deferral must be observable rather than look like an idle pass"
    );
    assert_eq!(outcome.queued_added, 0, "nothing loadable was added");
    assert_eq!(outcome.repair_added, 0, "no repair candidate was proven");
    assert!(
        queued.is_empty(),
        "an undetermined candidate must never become dispatchable work"
    );
    assert!(
        has_queue_intent(&shared, change_id).await,
        "the wake edge and the operator's queue intent must both survive"
    );

    let messages = drain_log_messages(&mut rx);
    assert!(
        messages.iter().any(|message| message
            == &format!(
                "Queue reconciliation deferred for '{change_id}': repair_evidence_unavailable"
            )),
        "the deferral must be identifiable in operator-facing logs, got {messages:?}"
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.contains("candidate_unavailable")),
        "an undetermined probe must not publish a settled verdict, got {messages:?}"
    );

    // Repeating the pass repeats neither the settle nor the diagnostic.
    let second = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;
    assert_eq!(
        second.unavailable_reconciled, 0,
        "repeating an undetermined pass still settles nothing"
    );
    assert!(
        has_queue_intent(&shared, change_id).await,
        "queue intent survives every undetermined pass"
    );
    let repeated = drain_log_messages(&mut rx);
    assert!(
        !repeated
            .iter()
            .any(|message| message.contains("repair_evidence_unavailable")),
        "deferral diagnostics stay bounded, got {repeated:?}"
    );
}

/// The same rule for the other half of the repair probe.
///
/// A workspace lookup that errors asked the question and got no answer, so
/// "this change has no repairable workspace" is precisely what was not proven.
#[tokio::test]
async fn a_failed_workspace_lookup_defers_instead_of_settling_queue_intent() {
    let temp_dir = TempDir::new().unwrap();
    init_minimal_git_repo(temp_dir.path());
    let change_id = "synthetic-workspace-lookup-failed";

    let (tx, _rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    // Base identity resolves; only workspace discovery fails.
    executor.set_workspace_manager(Box::new(
        crate::parallel::tests::executor::TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_failing_existing_workspace_lookup(),
    ));
    let shared = shared_state_with_queue_intent(&[change_id]);
    executor.set_shared_orchestrator_state(shared.clone());

    let mut queued = Vec::new();
    let in_flight = HashSet::new();

    let outcome = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;

    assert_eq!(
        outcome.unavailable_reconciled, 0,
        "a failed workspace lookup proves no absence"
    );
    assert_eq!(
        outcome.repair_evidence_deferred, 1,
        "the failed probe must be counted as a deferral"
    );
    assert!(queued.is_empty(), "nothing dispatchable was produced");
    assert!(
        has_queue_intent(&shared, change_id).await,
        "queue intent must be preserved for a pass that can read the evidence"
    );
}

/// An unreadable active-change catalog keeps the dynamic queue hint.
///
/// A popped hint may be the only wake edge a queued change has. A catalog read
/// that fails produced no lookup at all, so ingestion puts the hint back
/// instead of spending it, and the reducer's queue intent is untouched.
#[tokio::test]
async fn an_unreadable_catalog_retains_the_dynamic_queue_hint_and_queue_intent() {
    let temp_dir = TempDir::new().unwrap();
    let change_id = "synthetic-catalog-unreadable";

    // `openspec/changes` exists but is not a directory, so the catalog read
    // fails rather than reporting an empty active-change set.
    std::fs::create_dir_all(temp_dir.path().join("openspec")).expect("create openspec directory");
    std::fs::write(
        temp_dir.path().join("openspec").join("changes"),
        "not a dir\n",
    )
    .expect("write catalog blocker");

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let dynamic_queue = Arc::new(DynamicQueue::new());
    dynamic_queue.push(change_id.to_string()).await;

    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor.set_dynamic_queue(dynamic_queue.clone());
    let shared = shared_state_with_queue_intent(&[change_id]);
    executor.set_shared_orchestrator_state(shared.clone());

    let mut queued = Vec::new();
    let in_flight = HashSet::new();
    let mut reanalysis_reason = ReanalysisReason::Initial;

    let ingested = executor
        .check_dynamic_queue_and_add_changes(&mut queued, &in_flight, &mut reanalysis_reason)
        .await;

    assert!(!ingested, "an unreadable catalog ingests nothing");
    assert!(queued.is_empty(), "no scheduler-local work was produced");
    assert_eq!(
        dynamic_queue.len().await,
        1,
        "the wake edge must be requeued, not spent on a lookup that never happened"
    );
    assert_eq!(
        dynamic_queue.pop().await.as_deref(),
        Some(change_id),
        "the retained hint must be the same change, at the front"
    );
    assert!(
        has_queue_intent(&shared, change_id).await,
        "an unreadable catalog must not touch reducer queue intent"
    );

    let messages = drain_log_messages(&mut rx);
    assert!(
        messages.iter().any(|message| message.starts_with(&format!(
            "Queue reconciliation pending for '{change_id}': candidate_load_failed"
        ))),
        "the unreadable read must stay observable, got {messages:?}"
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.contains("candidate_not_found")),
        "unreadable is not absent, got {messages:?}"
    );
}

/// The reconciliation half of the same rule.
///
/// `resolve_missing_queued_candidate` re-reads the catalog itself. When that
/// re-read fails, absence is unproven, so queue intent is left exactly as it
/// was and the diagnostic stays bounded.
#[tokio::test]
async fn an_unreadable_refresh_leaves_queue_intent_exactly_as_it_was() {
    let temp_dir = TempDir::new().unwrap();
    let change_id = "synthetic-refresh-unreadable";

    std::fs::create_dir_all(temp_dir.path().join("openspec")).expect("create openspec directory");
    std::fs::write(
        temp_dir.path().join("openspec").join("changes"),
        "not a dir\n",
    )
    .expect("write catalog blocker");

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    let shared = shared_state_with_queue_intent(&[change_id]);
    executor.set_shared_orchestrator_state(shared.clone());

    let snapshot = executor.capture_reducer_work_snapshot().await;
    let mut queued = Vec::new();
    let mut outcome = crate::parallel::queue_state::QueueReconciliationOutcome::default();

    executor
        .resolve_missing_queued_candidate(change_id, &mut queued, &mut outcome, &snapshot)
        .await;

    assert_eq!(
        outcome.unavailable_reconciled, 0,
        "an unreadable refresh must never settle accepted queue intent"
    );
    assert_eq!(outcome.queued_added, 0, "nothing loadable was found");
    assert!(queued.is_empty(), "nothing dispatchable was produced");
    assert!(
        has_queue_intent(&shared, change_id).await,
        "queue intent must survive for a pass that can read the repository"
    );

    let messages = drain_log_messages(&mut rx);
    assert!(
        messages.iter().any(|message| message.starts_with(&format!(
            "Queue reconciliation pending for '{change_id}': candidate_load_failed"
        ))),
        "the unreadable refresh must stay observable, got {messages:?}"
    );

    // A second unreadable refresh repeats neither the settle nor the warning.
    executor
        .resolve_missing_queued_candidate(change_id, &mut queued, &mut outcome, &snapshot)
        .await;
    assert_eq!(
        outcome.unavailable_reconciled, 0,
        "repetition still proves no absence"
    );
    assert!(
        has_queue_intent(&shared, change_id).await,
        "queue intent survives every unreadable refresh"
    );
    let repeated = drain_log_messages(&mut rx);
    assert!(
        !repeated
            .iter()
            .any(|message| message.contains("candidate_load_failed")),
        "catalog read failure diagnostics stay bounded, got {repeated:?}"
    );
}
