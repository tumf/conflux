## Implementation Tasks

- [ ] 1. Update manual resolve event propagation so shared reducer state consumes `ResolveStarted`, `ResolveCompleted`, and failure/merge-terminal outcomes during TUI-driven resolve flow (verification: reducer-facing event path updated in `src/tui/runner.rs` and/or `src/tui/command_handlers.rs`)
- [ ] 2. Prevent stale reducer wait state from reapplying `ResolveWait` after successful merge completion on refresh (verification: regression path covered around `src/orchestration/state.rs` and `src/tui/state.rs`)
- [ ] 3. Add unit tests for the reducer/TUI sequence `MergeWait` -> `ResolveWait` -> successful resolve/merge -> `ChangesRefreshed` so the row remains terminal and never returns to `resolve pending` (verification: `cargo test` covering affected TUI/reducer tests)
- [ ] 4. Run `cargo fmt`, `cargo clippy -- -D warnings`, and targeted or full `cargo test` to confirm the stale-status fix passes repository checks (verification: commands complete successfully)

## Future Work

- Confirm whether Web monitoring should consume the same reducer-sync hardening for any equivalent stale wait display paths
