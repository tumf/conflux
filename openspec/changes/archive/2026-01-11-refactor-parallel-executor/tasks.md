## 1. Preparation

- [x] 1.1 Create `src/parallel/` directory
- [x] 1.2 Move common types to `src/parallel/types.rs` (WorkspaceResult)

## 2. Event-related Separation

- [x] 2.1 Move `ParallelEvent` enum to `src/parallel/events.rs`
- [x] 2.2 Move `send_event` helper method to events module

## 3. Cleanup Guard Separation

- [x] 3.1 Move `WorkspaceCleanupGuard` to `src/parallel/cleanup.rs`
- [x] 3.2 Include Drop implementation

## 4. Conflict Handling Separation

- [x] 4.1 Move `detect_conflicts` to `src/parallel/conflict.rs`
- [x] 4.2 Move `resolve_conflicts_with_retry`
- [x] 4.3 Move related helper functions

## 5. Execution Logic Separation

- [x] 5.1 Move `execute_apply_in_workspace` to `src/parallel/executor.rs`
- [x] 5.2 Move `execute_archive_in_workspace`
- [x] 5.3 Move `check_task_progress`

## 6. Orchestration Layer Organization

- [x] 6.1 Place remaining `ParallelExecutor` methods in `src/parallel/mod.rs`
- [x] 6.2 Delete `src/parallel_executor.rs`
- [x] 6.3 Re-export necessary types from `src/parallel/mod.rs`

## 7. Dependency Updates

- [x] 7.1 Update imports in `parallel_run_service.rs`
- [x] 7.2 Update imports in `tui/parallel_event_bridge.rs`
- [x] 7.3 Update other references (`orchestrator.rs`, `tui/orchestrator.rs`, `main.rs`)

## 8. Testing and Verification

- [x] 8.1 Tests moved to appropriate modules (events.rs, mod.rs)
- [x] 8.2 All tests pass with `cargo test` (264 tests)
- [x] 8.3 No warnings with `cargo clippy`
