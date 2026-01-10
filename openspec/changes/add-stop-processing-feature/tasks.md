# Tasks

## Phase 1: State Infrastructure

- [ ] 1.1. Add `StopMode` enum to `tui.rs` with `None`, `GracefulPending`, `ForceStopped` variants
- [ ] 1.2. Add `AppMode::Stopping` and `AppMode::Stopped` variants to existing enum
- [ ] 1.3. Add `stop_mode` field to `AppState` struct
- [ ] 1.4. Add `should_stop_after_current` flag for orchestrator communication

## Phase 2: Event Handling

- [ ] 2.1. Add Escape key handler in TUI event loop (only in Running/Stopping modes)
- [ ] 2.2. Implement graceful stop logic (first Esc): set `GracefulPending`, set flag
- [ ] 2.3. Implement force stop logic (second Esc): call `cancel_token.cancel()`
- [ ] 2.4. Handle `ProcessingCompleted` event during Stopping mode to transition to Stopped
- [ ] 2.5. Add new `OrchestratorEvent::Stopped` variant for communication

## Phase 3: Orchestrator Integration

- [ ] 3.1. Add stop flag check in `run_orchestrator` loop before picking next change
- [ ] 3.2. Send `Stopped` event when graceful stop completes
- [ ] 3.3. Handle cancelled state: return change to queued (not error)
- [ ] 3.4. Clear orchestrator_cancel token reference on stop

## Phase 4: Stopped Mode Functionality

- [ ] 4.1. Implement queue toggle in Stopped mode (Space key)
- [ ] 4.2. Implement resume processing (F5 key): reset state, start new orchestrator
- [ ] 4.3. Add warning for F5 with empty queue
- [ ] 4.4. Ensure q/Ctrl+C still work in Stopped mode

## Phase 5: UI Display

- [ ] 5.1. Update `render_header` to display "Stopping..." (yellow) and "Stopped" (gray)
- [ ] 5.2. Update help text for Running mode to include "Esc: stop"
- [ ] 5.3. Update help text for Stopping mode to show "Esc: force stop"
- [ ] 5.4. Update help text for Stopped mode to show "F5: resume, Space: toggle queue"
- [ ] 5.5. Update footer to show appropriate guidance in each stop state

## Phase 6: Testing

- [ ] 6.1. Add unit tests for `StopMode` state transitions
- [ ] 6.2. Add unit tests for `AppMode` transitions involving stop states
- [ ] 6.3. Add integration test for graceful stop flow
- [ ] 6.4. Add integration test for force stop flow
- [ ] 6.5. Add integration test for resume after stop
- [ ] 6.6. Manual testing: verify process termination works correctly

## Phase 7: Documentation

- [ ] 7.1. Update README with stop functionality description
- [ ] 7.2. Update help output if applicable
