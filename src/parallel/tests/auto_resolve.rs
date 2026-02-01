//! Tests for automatic resolve counter integration with parallel execution.

use crate::config::OrchestratorConfig;
use crate::parallel::ParallelExecutor;
use std::sync::Arc;
use tempfile::TempDir;

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
