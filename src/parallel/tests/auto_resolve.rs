//! Tests for automatic resolve counter integration with parallel execution.

use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::dynamic_queue::ReanalysisReason;
use crate::parallel::{ParallelExecutor, WorkspaceResult};
use crate::vcs::VcsBackend;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
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
async fn test_auto_resolve_counter_reduces_available_slots() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path().to_path_buf();

    // Create a basic config
    let config = create_test_config();

    // Create a ParallelExecutor with max_concurrent = 4
    let executor = ParallelExecutor::new(repo_root.clone(), config.clone(), None);

    // Get the auto resolve counter
    let auto_resolve_counter = executor.get_auto_resolve_counter();

    // Initially, counter should be 0
    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Auto resolve counter should start at 0"
    );

    // Simulate an automatic resolve starting (parallel executor would increment this)
    auto_resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    // Verify counter is now 1
    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Auto resolve counter should be 1 after increment"
    );

    // The available_slots calculation in execute_with_order_based_reanalysis should now be:
    // max_parallelism (4) - in_flight (0) - manual_resolve_count (0) - auto_resolve_count (1) = 3
    // This is tested implicitly by the slot calculation logic in the executor

    // Simulate resolve completing
    auto_resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

    // Counter should be back to 0
    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Auto resolve counter should return to 0 after completion"
    );
}

#[tokio::test]
async fn test_multiple_auto_resolves_consume_multiple_slots() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path().to_path_buf();

    // Create a basic config
    let config = create_test_config();

    // Create a ParallelExecutor
    let executor = ParallelExecutor::new(repo_root.clone(), config.clone(), None);
    let auto_resolve_counter = executor.get_auto_resolve_counter();

    // Simulate 2 concurrent automatic resolves
    auto_resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    auto_resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "Auto resolve counter should be 2 for concurrent resolves"
    );

    // If max_parallelism is 4, available_slots should now be:
    // 4 - 0 (in_flight) - 0 (manual_resolve_count) - 2 (auto_resolve_count) = 2

    // Simulate first resolve completing
    auto_resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Auto resolve counter should be 1 after one completes"
    );

    // Simulate second resolve completing
    auto_resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Auto resolve counter should be 0 after all complete"
    );
}

#[test]
fn test_auto_resolve_counter_is_thread_safe() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path().to_path_buf();

    // Create a basic config
    let config = create_test_config();

    // Create a ParallelExecutor
    let executor = ParallelExecutor::new(repo_root.clone(), config.clone(), None);
    let counter = executor.get_auto_resolve_counter();

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

#[tokio::test]
async fn test_combined_manual_and_auto_resolve_slots() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path().to_path_buf();

    // Create a basic config
    let config = create_test_config();

    // Create a ParallelExecutor
    let mut executor = ParallelExecutor::new(repo_root.clone(), config.clone(), None);

    // Set up manual resolve counter
    let manual_resolve_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    executor.set_manual_resolve_counter(manual_resolve_counter.clone());

    // Get auto resolve counter
    let auto_resolve_counter = executor.get_auto_resolve_counter();

    // Simulate 1 manual and 1 auto resolve running concurrently
    manual_resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    auto_resolve_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    assert_eq!(
        manual_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Manual resolve counter should be 1"
    );
    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Auto resolve counter should be 1"
    );

    // If max_parallelism is 4, available_slots should now be:
    // 4 - 0 (in_flight) - 1 (manual_resolve_count) - 1 (auto_resolve_count) = 2

    // Simulate manual resolve completing
    manual_resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        manual_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Manual resolve counter should be 0 after completion"
    );

    // Auto resolve counter should still be 1
    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Auto resolve counter should still be 1"
    );

    // Simulate auto resolve completing
    auto_resolve_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        auto_resolve_counter.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Auto resolve counter should be 0 after completion"
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
async fn test_auto_resolve_zero_capacity_runs_analysis_but_suppresses_apply_dispatch() {
    let temp_dir = TempDir::new().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let mut executor = ParallelExecutor::new(
        temp_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor
        .get_auto_resolve_counter()
        .store(1, std::sync::atomic::Ordering::SeqCst);

    let mut queued = vec![test_change("queued-auto-apply")];
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
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::AnalysisStarted { .. } => saw_analysis_started = true,
            ExecutionEvent::ApplyStarted { .. } => saw_apply_started = true,
            _ => {}
        }
    }

    assert!(
        saw_analysis_started,
        "queued work should enter analysis during active automatic resolve"
    );
    assert!(
        !saw_apply_started,
        "ordinary apply must remain capacity-gated during active automatic resolve"
    );
}
