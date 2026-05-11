## Implementation Tasks

- [x] Update TUI refresh merge-wait display synchronization so `merge_wait_ids` does not overwrite a reducer-owned `resolve pending` status for the same change. (verification: unit - add or update tests around `src/tui/state/event_handlers/refresh.rs` / `src/tui/state.rs`; completion condition: the test fails if `apply_refresh_merge_wait_status` blindly changes accepted `ResolveWait` rows back to `merge wait`)
- [x] Preserve stale-display correction for local `resolve pending` rows that have no reducer-owned `ResolveWait`. (verification: unit - add a TUI refresh regression in `src/tui/state/event_handlers/refresh.rs` or `src/tui/state.rs` where the local row starts as `resolve pending`, reducer display is absent or non-`resolve pending`, `merge_wait_ids` includes the change, and the final display is `merge wait`; completion condition: stale display-only pending is still corrected)
- [x] Preserve terminal row protection when refresh evidence includes `merge_wait_ids`. (verification: unit - keep or extend terminal `merged`/`rejected` refresh tests in `src/tui/state/event_handlers/refresh.rs` or `src/tui/state.rs`; completion condition: terminal rows do not regress to `merge wait`)
- [x] Verify accepted manual `M` retry lifecycle remains scheduler-owned and observable. (verification: unit - run focused tests covering `ResolveMerge`, `ResolveWait`, and refresh survival, such as `cargo test resolve_wait_survives_changes_refreshed resolve_merge_immediate_syncs_reducer`; completion condition: accepted reducer intent remains present until scheduler lifecycle events clear it)
- [x] Run formatting/lint/typecheck/test commands required for touched Rust/TUI code. (verification: manual - ran `cargo fmt`, `cargo fmt --check`, `cargo test refresh --lib`, `cargo test resolve_wait_survives_changes_refreshed --lib`, `cargo test resolve_merge_immediate_syncs_reducer --lib`, and `cargo clippy --lib --tests -- -D warnings`; completion condition: commands pass or any unrelated failures are documented with evidence)

## Future Work

- Manual TUI smoke test in a real parallel run: archive a change into `merge wait`, press `M`, observe that the row does not flicker back to `merge wait` on the next periodic refresh while the reducer owns `ResolveWait`.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-manual-resolve-refresh-regression --archive-gate`
