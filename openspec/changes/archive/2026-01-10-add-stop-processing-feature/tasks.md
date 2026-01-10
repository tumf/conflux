# Tasks

## Phase 1: State Infrastructure

- [x] 1.1. Add `StopMode` enum to `tui.rs` with `None`, `GracefulPending`, `ForceStopped` variants
- [x] 1.2. Add `AppMode::Stopping` and `AppMode::Stopped` variants to existing enum
- [x] 1.3. Add `stop_mode` field to `AppState` struct
- [x] 1.4. Add shared `graceful_stop_flag` (`Arc<AtomicBool>`) for orchestrator communication

## Phase 2: Event Handling

- [x] 2.1. Add Escape key handler in TUI event loop (only in Running/Stopping modes)
- [x] 2.2. Implement graceful stop logic (first Esc): set `GracefulPending`, set flag
- [x] 2.3. Implement force stop logic (second Esc): call `cancel_token.cancel()`
- [x] 2.4. Add new `OrchestratorEvent::Stopped` variant for communication

## Phase 3: Orchestrator Integration

- [x] 3.1. Add stop flag check in `run_orchestrator` loop before picking next change
- [x] 3.2. Send `Stopped` event when graceful stop completes

## Phase 4: Stopped Mode Functionality

- [x] 4.1. Implement queue toggle in Stopped mode (Space key)
- [x] 4.2. Implement resume processing (F5 key): reset state, start new orchestrator
- [x] 4.3. Add warning for F5 with empty queue

## Phase 5: UI Display

- [x] 5.1. Update `render_header` to display "Stopping..." (yellow) and "Stopped" (gray)
- [x] 5.2. Update help text for Running mode to include "Esc: stop"
- [x] 5.3. Update help text for Stopping mode to show "Esc: force stop"
- [x] 5.4. Update help text for Stopped mode to show "F5: resume, Space: toggle queue"
