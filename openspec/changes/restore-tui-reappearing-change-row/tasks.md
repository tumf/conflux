## Implementation Tasks

- [ ] Reconcile `known_change_ids` with the IDs present in each successful active and rejected refresh snapshot, allowing a later reappearance to be classified and reconstructed without changing workflow-control state (verification: unit - `cargo test changes_refreshed_restores_change_after_transient_absence -- --exact`; verification-id: refresh-reappearance-test)
- [ ] Add the `present → absent → present` regression test and assert final row restoration, current-data reconstruction, cursor stability, unselected state, active new-change count/log behavior, and rejected-row invariants (verification: unit - `cargo test changes_refreshed_restores_change_after_transient_absence -- --exact`; verification-id: refresh-reappearance-test)

## Future Work

- None.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate restore-tui-reappearing-change-row --archive-gate`
