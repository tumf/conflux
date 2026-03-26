## Implementation Tasks

- [x] 1. Update manual resolve event propagation so shared reducer state consumes `ResolveStarted`, `ResolveCompleted`, and failure/merge-terminal outcomes during TUI-driven resolve flow (verification: reducer-facing event path updated in `src/tui/runner.rs` — now applies ResolveStarted/ResolveCompleted/ResolveFailed alongside ChangesRefreshed)
- [x] 2. Prevent stale reducer wait state from reapplying `ResolveWait` after successful merge completion on refresh (verification: `src/orchestration/state.rs` `apply_execution_event` for `ResolveCompleted` now sets `terminal = Merged`; `apply_observation` skips terminal entries so stale wait cannot revive; `ResolveFailed` restores `MergeWait`)
- [x] 3. Add unit tests for the reducer/TUI sequence `MergeWait` -> `ResolveWait` -> successful resolve/merge -> `ChangesRefreshed` so the row remains terminal and never returns to `resolve pending` (verification: `cargo test` — `test_resolve_completed_clears_resolve_wait_and_survives_refresh` and `test_resolve_failed_restores_merge_wait` both pass; 37/37 orchestration state tests pass)
- [x] 4. Run `cargo fmt`, `cargo clippy -- -D warnings`, and targeted or full `cargo test` to confirm the stale-status fix passes repository checks (verification: all commands completed successfully — 1203 tests pass, 0 failures)

## Future Work

- Confirm whether Web monitoring should consume the same reducer-sync hardening for any equivalent stale wait display paths
