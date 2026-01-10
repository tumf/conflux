# Tasks: Refactor tui.rs Module Structure

## 1. Preparation

- [ ] 1.1 Create `src/tui/` directory structure
- [ ] 1.2 Create `src/tui/mod.rs` with placeholder re-exports
- [ ] 1.3 Run `cargo check` to ensure baseline compiles

## 2. Extract Types Module

- [ ] 2.1 Create `src/tui/types.rs` with:
  - `StopMode` enum
  - `AppMode` enum
  - `QueueStatus` enum and impl
- [ ] 2.2 Update `mod.rs` to re-export types
- [ ] 2.3 Run `cargo check`

## 3. Extract Queue Module

- [ ] 3.1 Create `src/tui/queue.rs` with:
  - `DynamicQueue` struct and impl
- [ ] 3.2 Update `mod.rs` to re-export queue
- [ ] 3.3 Run `cargo check`

## 4. Extract Events Module

- [ ] 4.1 Create `src/tui/events.rs` with:
  - `LogEntry` struct and impl
  - `TuiCommand` enum
  - `OrchestratorEvent` enum
- [ ] 4.2 Update `mod.rs` to re-export events
- [ ] 4.3 Run `cargo check`

## 5. Extract State Module

- [ ] 5.1 Create `src/tui/state.rs` with:
  - `ChangeState` struct and impl
  - `AppState` struct and impl
- [ ] 5.2 Update internal imports to use types and events modules
- [ ] 5.3 Update `mod.rs` to re-export state
- [ ] 5.4 Run `cargo check`

## 6. Extract Utils Module

- [ ] 6.1 Create `src/tui/utils.rs` with:
  - `truncate_to_display_width` function
  - `clear_screen` function
  - `launch_editor_for_change` function
  - `get_version_string` function
- [ ] 6.2 Update `mod.rs` to re-export utils (where needed)
- [ ] 6.3 Run `cargo check`

## 7. Extract Render Module

- [ ] 7.1 Create `src/tui/render.rs` with:
  - `render` function (main entry point)
  - `render_select_mode` function
  - `render_running_mode` function
  - `render_header` function
  - `render_changes_list_select` function
  - `render_changes_list_running` function
  - `render_status` function
  - `render_logs` function
  - `render_footer_select` function
  - Constants: `SPINNER_CHARS`
- [ ] 7.2 Update imports and function visibility
- [ ] 7.3 Run `cargo check`

## 8. Extract Orchestrator Module

- [ ] 8.1 Create `src/tui/orchestrator.rs` with:
  - `ArchiveContext` struct
  - `ArchiveResult` enum
  - `archive_single_change` function
  - `archive_all_complete_changes` function
  - `run_orchestrator` function
- [ ] 8.2 Update imports to use types, events, and state modules
- [ ] 8.3 Run `cargo check`

## 9. Extract Runner Module

- [ ] 9.1 Create `src/tui/runner.rs` with:
  - `run_tui` function (public entry point)
  - `run_tui_loop` function
  - Constants: `AUTO_REFRESH_INTERVAL_SECS`, `MAX_LOG_ENTRIES`
- [ ] 9.2 Update imports to use all other modules
- [ ] 9.3 Run `cargo check`

## 10. Finalize Module Structure

- [ ] 10.1 Update `src/tui/mod.rs` with final re-exports
- [ ] 10.2 Remove old `src/tui.rs` file
- [ ] 10.3 Update any external imports in other source files
- [ ] 10.4 Run `cargo check`

## 11. Migrate Tests

- [ ] 11.1 Create `src/tui/tests.rs` or inline test modules in each submodule
- [ ] 11.2 Move tests to appropriate modules based on what they test
- [ ] 11.3 Update test imports
- [ ] 11.4 Run `cargo test` to verify all tests pass

## 12. Final Validation

- [ ] 12.1 Run `cargo fmt`
- [ ] 12.2 Run `cargo clippy -- -D warnings`
- [ ] 12.3 Run `cargo test`
- [ ] 12.4 Run `cargo build --release`
- [ ] 12.5 Manual TUI test: `cargo run -- tui`
