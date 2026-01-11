## 1. Preparation

- [x] 1.1 Create `src/tui/state/` directory
- [x] 1.2 Set up basic module structure (`mod.rs`)

## 2. ChangeState Separation

- [x] 2.1 Move `ChangeState` struct to `src/tui/state/change.rs`
- [x] 2.2 Move related methods (`from_change`, `progress_percent`, etc.)
- [x] 2.3 Move `ChangeState` tests

## 3. Mode-related Separation

- [x] 3.1 `AppMode` enum already exists in `types.rs` (no move needed)
- [x] 3.2 Move `start_processing`, `toggle_parallel_mode`, `retry_error_changes` to `modes.rs`
- [x] 3.3 Move mode-related tests

## 4. Log Management Separation

- [x] 4.1 Move log-related constant (`MAX_LOG_ENTRIES`) to `src/tui/state/logs.rs`
- [x] 4.2 Move `add_log`, `scroll_logs_*` methods
- [x] 4.3 Log-related tests (none existed in original)

## 5. Event Handling Separation

- [x] 5.1 Move `handle_orchestrator_event` to `src/tui/state/events.rs`
- [x] 5.2 Move `update_changes` method
- [x] 5.3 Move event-related tests

## 6. Main Module Cleanup

- [x] 6.1 Place remaining `AppState` methods in `src/tui/state/mod.rs`
- [x] 6.2 Re-export necessary types (`ChangeState`)
- [x] 6.3 Delete `src/tui/state.rs`

## 7. Dependency Updates

- [x] 7.1 Update imports in `src/tui/mod.rs` (no changes needed - Rust auto-resolves)
- [x] 7.2 Update imports in `src/tui/render.rs` (no changes needed)
- [x] 7.3 Update imports in `src/tui/runner.rs` (no changes needed)

## 8. Testing and Verification

- [x] 8.1 All 246 unit tests pass with `cargo test`
- [x] 8.2 No warnings with `cargo clippy`
- [x] 8.3 TUI startup verification (skipped - CI environment)
