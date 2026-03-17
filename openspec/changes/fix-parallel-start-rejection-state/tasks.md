## Implementation Tasks

- [ ] Update the parallel-start rejection flow in `src/parallel_run_service.rs` and related event types so rejected changes are surfaced to callers as explicit start-time reconciliation data rather than a silent early return (verification: backend filtering path distinguishes "rejected before start" from "completed successfully").
- [ ] Update TUI parallel start/resume handling in `src/tui/state.rs`, `src/tui/orchestrator.rs`, and related event handling so backend-rejected changes return to a non-queued state with a user-visible reason (verification: `F5` and stopped-mode resume no longer leave rejected rows in `Queued`).
- [ ] Update CLI parallel run reporting in `src/orchestrator.rs` so `cflx run --parallel` clearly reports when zero changes started because backend eligibility filtering rejected them all (verification: CLI output distinguishes no-op rejection from real execution progress/completion).
- [ ] Add regression tests covering TUI stale eligibility and CLI all-rejected parallel start behavior (verification: targeted tests in TUI/parallel/orchestrator modules fail before the fix and pass after it).
- [ ] Run proposal-aligned verification after implementation (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, and relevant `cargo test` coverage for the new rejection-path regressions).

## Future Work

- Consider introducing a dedicated shared status such as "RejectedAtStart" if future UX needs stronger differentiation than warnings plus restored idle/not-queued state.
