## Implementation Tasks

- [ ] Update `src/tui/state/event_handlers/refresh.rs` so `handle_changes_refreshed` consumes `merge_wait_ids` and applies `merge wait` to matching TUI rows after ordinary change/worktree refresh processing. (verification: unit - add a refresh-handler test that starts a change row at `resolve pending`, calls `handle_changes_refreshed` with that id in `merge_wait_ids`, and asserts `display_status_cache == "merge wait"`; completion condition: the test fails when `merge_wait_ids` is ignored and passes after the handler change)
- [ ] Preserve terminal display states when applying refresh-derived merge-wait evidence. (verification: unit - add a TUI refresh-handler test for at least one terminal row such as `merged` or `rejected` included in `merge_wait_ids`, asserting the terminal display status is not regressed to `merge wait`; completion condition: stale refresh evidence cannot overwrite terminal TUI rows)
- [ ] Keep `merge_wait_ids` display-only and non-authoritative for workflow decisions. (verification: integration - inspect `src/tui/state/event_handlers/refresh.rs`, `src/tui/runner.rs`, `src/orchestration/state.rs`, and `src/parallel/queue_state.rs`; run focused tests for TUI refresh and reducer merge-wait behavior such as `cargo test merge_wait` or narrower repository-supported filters; completion condition: no scheduler dispatch, archive, acceptance, or resume routing branch reads TUI-local merge-wait display state)
- [ ] Run repository verification for touched Rust code. (verification: integration - run focused Rust tests covering added TUI refresh tests plus existing `ChangesRefreshed`/`MergeDeferred` reducer tests, and run the repository lint/typecheck commands when configured; completion condition: commands pass or failures are documented with exact output and rationale)

## Future Work

- If Web UI status rendering shows the same mismatch independently, create a separate proposal scoped to Web UI state synchronization.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-merge-wait-refresh-display --archive-gate`.
