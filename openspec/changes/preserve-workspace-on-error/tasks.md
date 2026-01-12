# Implementation Tasks

## 1. Core Logic Implementation

- [x] 1.1 `src/parallel/mod.rs` - Modified cleanup logic to skip failed workspaces
- [x] 1.2 Added workspace name logging on error (`Failed for {change_id}, workspace preserved: {workspace_name}`)
- [x] 1.3 Added recovery hint message at INFO level (`To resume: run with the same change_id, workspace will be automatically detected`)
- [x] 1.4 `src/parallel/cleanup.rs` - Modified `CleanupGuard` with `preserve()` method and preserved workspace tracking
- [x] 1.5 `WorkspaceResult` already contains `workspace_name` field (no change needed)

## 2. Event Notification

- [x] 2.1 Added `WorkspacePreserved { change_id, workspace_name }` event to `ParallelEvent`
- [x] 2.2 Added TUI event bridge handling in `parallel_event_bridge.rs` to display preserved workspace warnings

## 3. Tests

- [x] 3.1 Added tests for workspace preservation in `cleanup.rs`:
  - `test_cleanup_guard_preserve_workspace`
  - `test_cleanup_guard_preserved_workspace_not_cleaned_on_drop`
  - `test_cleanup_guard_all_preserved_does_nothing`
  - `test_cleanup_guard_preserved_workspaces_starts_empty`
- [x] 3.2 Existing tests confirm successful workspaces are cleaned up normally
- [x] 3.3 Added test for `WorkspacePreserved` event conversion in `parallel_event_bridge.rs`

## 4. Validation

- [x] 4.1 `cargo fmt` - Code formatted
- [x] 4.2 `cargo clippy` - No warnings
- [x] 4.3 `cargo test` - All 450 tests passing (421 unit + 26 e2e + 3 compatibility)
- [ ] 4.4 Manual verification with `add-workspace-resume` integration (requires runtime testing)
