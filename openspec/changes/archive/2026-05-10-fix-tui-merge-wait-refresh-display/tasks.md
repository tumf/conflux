## Implementation Tasks

- [x] Update `src/tui/state/event_handlers/refresh.rs` so `handle_changes_refreshed` consumes `merge_wait_ids` and applies `merge wait` to matching TUI rows after ordinary change/worktree refresh processing. (verification: unit - add a test in `src/tui/state/event_handlers/refresh.rs` or `src/tui/state.rs`, expected filter `cargo test merge_wait_refresh`, that starts a change row at `resolve pending`, calls `handle_changes_refreshed` with that id in `merge_wait_ids`, and asserts `display_status_cache == "merge wait"`; completion condition: the test fails when `merge_wait_ids` is ignored and passes after the handler change)
- [x] Preserve terminal display states when applying refresh-derived merge-wait evidence. (verification: unit - add a test in `src/tui/state/event_handlers/refresh.rs` or `src/tui/state.rs`, expected filter `cargo test merge_wait_refresh`, for at least one terminal row such as `merged` or `rejected` included in `merge_wait_ids`, asserting the terminal display status is not regressed to `merge wait`; completion condition: stale refresh evidence cannot overwrite terminal TUI rows)
- [x] Keep `merge_wait_ids` display-only and non-authoritative for workflow decisions. (verification: integration - inspect `src/tui/state/event_handlers/refresh.rs`, `src/tui/runner.rs`, `src/orchestration/state.rs`, and `src/parallel/queue_state.rs`, then run focused tests such as `cargo test merge_wait` plus the new `cargo test merge_wait_refresh`; completion condition: no scheduler dispatch, archive, acceptance, or resume routing branch reads TUI-local `display_status_cache` or refresh-applied merge-wait UI state)
- [x] Run repository verification for touched Rust code. (verification: integration - run `cargo test merge_wait_refresh`, `cargo test merge_wait`, and the repository lint/typecheck commands configured in `Cargo.toml` or project docs when available, such as `cargo fmt --check`/`cargo clippy`; completion condition: commands pass or failures are documented with exact output and rationale)

## Future Work

- If Web UI status rendering shows the same mismatch independently, create a separate proposal scoped to Web UI state synchronization.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-merge-wait-refresh-display --archive-gate`.
