## Implementation Tasks

- [ ] Update the parallel TUI queue-state synchronization logic so reducer-owned queued intent survives the initial post-`F5` refresh before analysis starts (verification: `src/tui/state.rs` and `src/orchestration/state.rs` preserve queued display through refresh-driven sync).
- [ ] Ensure startup rejection paths are the only pre-analysis path that return an explicitly queued row to `not queued` (verification: `src/tui/state.rs` rejection handling and related command/orchestrator flow remain aligned).
- [ ] Add or update regression tests covering the `F5` -> queued -> refresh path in parallel mode and any rejection exception path (verification: `cargo test test_parallel_start_refresh_preserves_queued_rows` and related TUI state tests).
- [ ] Run repository validation for the affected behavior (verification: `cargo test`, or at minimum the targeted TUI state tests covering queue synchronization).
