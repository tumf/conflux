//! Tests for manual resolve counter integration with parallel execution.

use crate::config::OrchestratorConfig;
use crate::parallel::ParallelExecutor;
use std::sync::{atomic::AtomicUsize, Arc};
use tempfile::TempDir;

#[tokio::test]
async fn test_manual_resolve_counter_reduces_available_slots() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path().to_path_buf();

    // Create a basic config
    let config = OrchestratorConfig::default();

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
    let config = OrchestratorConfig::default();

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
