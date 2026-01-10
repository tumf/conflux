# Tasks: Refactor tui.rs Module Structure

## 1. Setup Module Structure

- [x] 1.1 Create `src/tui/` directory and `mod.rs` with placeholder re-exports

## 2. Extract Core Modules

- [x] 2.1 Create `src/tui/types.rs` (StopMode, AppMode, QueueStatus enums)
- [x] 2.2 Create `src/tui/queue.rs` (DynamicQueue struct and impl)
- [x] 2.3 Create `src/tui/events.rs` (LogEntry, TuiCommand, OrchestratorEvent)

## 3. Extract State and Utils

- [x] 3.1 Create `src/tui/state.rs` (ChangeState, AppState structs and impls)
- [x] 3.2 Create `src/tui/utils.rs` (truncate_to_display_width, clear_screen, launch_editor_for_change, get_version_string)

## 4. Extract UI and Logic Modules

- [x] 4.1 Create `src/tui/render.rs` (all render_* functions, SPINNER_CHARS)
- [x] 4.2 Create `src/tui/orchestrator.rs` (ArchiveContext, ArchiveResult, archive functions, run_orchestrator)
- [x] 4.3 Create `src/tui/runner.rs` (run_tui, run_tui_loop, constants)

## 5. Finalize

- [x] 5.1 Update `src/tui/mod.rs` with final re-exports
- [x] 5.2 Remove old `src/tui.rs` and update external imports
- [x] 5.3 Migrate tests to appropriate submodules
