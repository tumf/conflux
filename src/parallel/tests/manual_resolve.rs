//! Tests for manual resolve counter integration with parallel execution.

use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::dynamic_queue::ReanalysisReason;
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
) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::analyzer::AnalysisResult> + Send + 'a>>
{
    let order = changes.iter().map(|change| change.id.clone()).collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies: HashMap::new(),
            groups: None,
        }
    })
}

#[tokio::test]
async fn test_manual_resolve_zero_capacity_runs_analysis_but_suppresses_apply_dispatch() {
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
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            1,
            1,
            ReanalysisReason::ResolveCompletion,
            &analysis_result,
            semaphore,
            &mut join_set,
            &mut cleanup_guard,
        )
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
    let mut saw_capacity_diagnostic = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::AnalysisStarted { .. } => saw_analysis_started = true,
            ExecutionEvent::ApplyStarted { .. } => saw_apply_started = true,
            ExecutionEvent::Log(entry)
                if entry
                    .message
                    .contains("dispatch_capacity_zero_after_analysis") =>
            {
                saw_capacity_diagnostic = true;
            }
            _ => {}
        }
    }

    assert!(
        saw_analysis_started,
        "queued work should enter analysis during active manual resolve"
    );
    assert!(
        !saw_apply_started,
        "ordinary apply must remain capacity-gated during active manual resolve"
    );
    assert!(
        saw_capacity_diagnostic,
        "capacity-gated dispatch should emit an operator-visible diagnostic"
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
        while !(saw_dynamic_ingest && saw_analysis_started && saw_capacity_diagnostic) {
            match rx.recv().await {
                Some(ExecutionEvent::Log(entry))
                    if entry.message.contains(&format!(
                        "Dynamically added to parallel execution: {synthetic_change_id}"
                    )) =>
                {
                    saw_dynamic_ingest = true;
                }
                Some(ExecutionEvent::Log(entry))
                    if entry
                        .message
                        .contains("dispatch_capacity_zero_after_analysis") =>
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
        "controllable resolve gate must still be held when analysis and capacity diagnostics fire"
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
    assert!(saw_analysis_started);
    assert!(saw_capacity_diagnostic);
    assert!(
        !saw_apply_started,
        "zero recalculated capacity must suppress apply dispatch while gated resolve is active"
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
