# Tasks: Refactor Parallel Executor

## 1. Extract ParallelEventBridge
- [x] 1.1 Create `src/tui/parallel_event_bridge.rs` with `convert()` function
- [x] 1.2 Update `src/tui/orchestrator.rs` to use `ParallelEventBridge::convert()`
- [x] 1.3 Remove old inline match block from orchestrator

## 2. Add WorkspaceCleanupGuard
- [x] 2.1 Implement `WorkspaceCleanupGuard` struct in `src/parallel_executor.rs`
- [x] 2.2 Integrate guard into `execute_group()` method

## 3. Extract Apply Loop Helpers
- [x] 3.1 Extract `check_task_progress()` helper function
- [x] 3.2 Extract `summarize_output()` helper function
- [x] 3.3 Refactor `execute_apply_in_workspace()` to use helpers

## 4. Create ParallelRunService
- [x] 4.1 Create `src/parallel_run_service.rs` with service struct
- [x] 4.2 Implement `run_parallel()` method with callback-based events
- [x] 4.3 Migrate CLI `run_parallel()` to use service
- [x] 4.4 Migrate TUI `run_orchestrator_parallel()` to use service
