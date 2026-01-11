# Tasks: Refactor Parallel Executor

## 1. Extract ParallelEventBridge
- [ ] 1.1 Create `src/tui/parallel_event_bridge.rs` with `convert()` function
- [ ] 1.2 Add unit tests for each event type mapping
- [ ] 1.3 Update `src/tui/orchestrator.rs` to use `ParallelEventBridge::convert()`
- [ ] 1.4 Remove old inline match block from orchestrator

## 2. Add WorkspaceCleanupGuard
- [ ] 2.1 Implement `WorkspaceCleanupGuard` struct in `src/parallel_executor.rs`
- [ ] 2.2 Add unit tests for guard behavior (commit/drop scenarios)
- [ ] 2.3 Integrate guard into `execute_group()` method

## 3. Extract Apply Loop Helpers
- [ ] 3.1 Extract `check_task_progress()` helper function
- [ ] 3.2 Extract `summarize_output()` helper function
- [ ] 3.3 Add unit tests for extracted helpers
- [ ] 3.4 Refactor `execute_apply_in_workspace()` to use helpers

## 4. Create ParallelRunService
- [ ] 4.1 Create `src/parallel_run_service.rs` with service struct
- [ ] 4.2 Implement `run_parallel()` method with callback-based events
- [ ] 4.3 Add unit tests for service behavior
- [ ] 4.4 Migrate CLI `run_parallel()` to use service
- [ ] 4.5 Migrate TUI `run_orchestrator_parallel()` to use service

## 5. Cleanup
- [ ] 5.1 Remove duplicate `group_by_dependencies()` implementations
