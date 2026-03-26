## Implementation Tasks

- [x] Update the parallel TUI queue-state synchronization logic so reducer-owned queued intent survives the initial post-`F5` refresh before analysis starts (verification: `src/tui/orchestrator.rs` re-applies `AddToQueue` for each selected change after resetting shared state in `run_orchestrator_parallel`).
- [x] Ensure startup rejection paths are the only pre-analysis path that return an explicitly queued row to `not queued` (verification: `src/tui/state.rs` rejection handling via `handle_parallel_start_rejected` remains the sole pre-analysis path that regresses to `NotQueued`).
- [x] Add or update regression tests covering the `F5` -> queued -> refresh path in parallel mode and any rejection exception path (verification: `cargo test test_parallel_start_refresh_preserves_queued_rows` passes; new `test_parallel_start_state_reset_preserves_queued_rows` added and passing).
- [x] Run repository validation for the affected behavior (verification: `cargo test tui::state` — 77/77 passed; `cargo test orchestration::state` — 37/37 passed).
