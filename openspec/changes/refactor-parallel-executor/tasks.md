# Tasks: Refactor Parallel Executor

## 1. Preparation
- [ ] 1.1 Add integration test for CLI parallel execution flow
- [ ] 1.2 Add integration test for TUI parallel execution flow
- [ ] 1.3 Document current parallel execution behavior as baseline

## 2. Extract ParallelEventBridge
- [ ] 2.1 Create `src/tui/parallel_event_bridge.rs` with `convert()` function
- [ ] 2.2 Add unit tests for each event type mapping
- [ ] 2.3 Update `src/tui/orchestrator.rs` to use `ParallelEventBridge::convert()`
- [ ] 2.4 Remove old inline match block from orchestrator

## 3. Add WorkspaceCleanupGuard
- [ ] 3.1 Implement `WorkspaceCleanupGuard` struct in `src/parallel_executor.rs`
- [ ] 3.2 Add unit tests for guard behavior (commit/drop scenarios)
- [ ] 3.3 Integrate guard into `execute_group()` method
- [ ] 3.4 Verify cleanup on simulated failure

## 4. Extract Apply Loop Helpers
- [ ] 4.1 Extract `check_task_progress()` helper function
- [ ] 4.2 Extract `summarize_output()` helper function
- [ ] 4.3 Add unit tests for extracted helpers
- [ ] 4.4 Refactor `execute_apply_in_workspace()` to use helpers

## 5. Create ParallelRunService
- [ ] 5.1 Create `src/parallel_run_service.rs` with service struct
- [ ] 5.2 Implement `run_parallel()` method with callback-based events
- [ ] 5.3 Add unit tests for service behavior
- [ ] 5.4 Migrate CLI `run_parallel()` to use service
- [ ] 5.5 Migrate TUI `run_orchestrator_parallel()` to use service

## 6. Cleanup and Verification
- [ ] 6.1 Remove duplicate `group_by_dependencies()` implementations
- [ ] 6.2 Run all tests and verify no regressions
- [ ] 6.3 Run `cargo clippy` and fix any warnings
- [ ] 6.4 Update module documentation
