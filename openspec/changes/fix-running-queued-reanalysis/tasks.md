## Implementation Tasks

- [x] Task 1: Update parallel scheduler trigger rules so queued changes are re-analyzed immediately when available execution slots transition from zero to positive (files: `src/parallel/orchestration.rs`, `src/parallel/queue_state.rs`; verification: new unit/integration test demonstrates slot-release-triggered reanalysis without waiting for debounce).
- [x] Task 2: Propagate manual resolve completion as a scheduler-visible reanalysis trigger using the same slot accounting model as other active parallel work (files: `src/tui/command_handlers.rs`, `src/parallel/orchestration.rs`, `src/parallel/queue_state.rs`; verification: regression test covers `MergeWait` -> manual resolve -> queued follow-up changes dispatching after resolve completion).
- [x] Task 3: Preserve debounce only for queue-edit bursts that do not involve slot-availability recovery, and add regression coverage for both debounced and immediate paths (files: `src/parallel/dynamic_queue.rs`, `src/parallel/queue_state.rs`, related tests under `src/parallel/tests/`; verification: tests cover both debounce-preserved and debounce-bypassed scenarios).
- [x] Task 4: Align TUI-visible queued/running state transitions and logs with the new scheduling behavior so queued rows are not misleadingly idle after capacity returns (files: `src/tui/state.rs`, `src/tui/render.rs`, `src/tui/command_handlers.rs`; verification: render/state tests confirm queued rows remain consistent and reflect prompt dispatch behavior).
- [x] Task 5: Add or update regression tests for the reported scenario: start one `MergeWait` resolve, queue two additional changes during `Running`, then verify analyze/dispatch resumes when the slot becomes free (files: `src/parallel/tests/`, `src/tui/state.rs` tests, or equivalent focused test modules; verification: targeted `cargo test` cases for the scenario).

## Future Work

- If users still perceive ambiguity in waiting states, consider a separate proposal for richer UI wording such as explicit `waiting for slot` vs `waiting for debounce` indicators.
