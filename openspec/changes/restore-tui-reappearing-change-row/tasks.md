## Implementation Tasks

- [ ] Reconcile `known_change_ids` in `src/tui/state/processing_logic.rs` with the row projection that survives each successful refresh — an ID leaves the set when its row is removed, and stays known while its row is retained despite snapshot absence — so a later reappearance is classified and reconstructed without duplicate rows and without changing workflow-control state (verification: unit - `cargo test tui::state::event_handlers::refresh::tests::changes_refreshed_restores_change_after_transient_absence -- --exact`; verification-id: refresh-reappearance-test)
- [ ] Add the `present → absent → present` regression test and assert final row restoration, current-data reconstruction, cursor stability, unselected state, active new-change count/log behavior, rejected-row invariants, and that a row retained through an absence is updated in place when re-observed (no duplicate row, no NEW badge, no detection log) (verification: unit - `cargo test tui::state::event_handlers::refresh::tests::changes_refreshed_restores_change_after_transient_absence -- --exact`; verification-id: refresh-reappearance-test)

## Future Work

- None.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate restore-tui-reappearing-change-row --archive-gate`
