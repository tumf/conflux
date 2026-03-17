## Implementation Tasks

- [ ] Align `F5` start/resume state transitions with authoritative parallel eligibility in `src/tui/state.rs`, `src/tui/command_handlers.rs`, and/or `src/tui/orchestrator.rs` so rows rejected at parallel start do not remain `Queued` (verification: code paths no longer leave rejected rows queued after backend filtering).
- [ ] Update parallel-start filtering in `src/parallel_run_service.rs` and related event flow so rejected uncommitted changes emit enough information for the TUI to restore a non-queued state and surface the reason to the user (verification: parallel-start early-return path produces state reconciliation instead of silent `Ok(())`).
- [ ] Add regression tests covering stale eligibility on `F5` from Select and/or Stopped mode, including the case where the TUI cached eligibility differs from the latest Git check (verification: relevant unit/integration tests under `src/tui/state.rs` and/or parallel execution tests fail before the fix and pass after it).
- [ ] Run proposal-aligned verification for formatting and tests after implementation (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, and targeted `cargo test` coverage for the new regression case).

## Future Work

- Consider exposing a first-class TUI status for "rejected at start" if users need stronger differentiation than warning logs plus a return to `NotQueued`.
