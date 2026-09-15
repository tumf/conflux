## Implementation Tasks

- [ ] Transfer the owned execution permit from workspace dispatch through background merge settlement, with no release between archive completion and merge task admission. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [ ] Make scheduler capacity membership represent each admitted change exactly once through apply, acceptance, archive, merge, automatic resolve, and manual resolve. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [ ] Retain lifecycle occupancy in `merge_wait`, then release it only after merged, terminal error/rejection, explicit dequeue, or configured push/publication terminal settlement. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [ ] Preserve capacity recovery wakeups when terminal settlement releases a slot, without polling or sticky-trigger replay. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [ ] Reconstruct lifecycle occupancy after restart from workspace, Git, and reducer evidence without durable out-of-worktree slot state. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [ ] Add `parallel::tests::lifecycle_slot_ownership` regression coverage for archive-to-merge transfer, auto resolve, full-capacity manual resolve, merge wait, terminal release, and TUI/API count parity. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate fix-lifecycle-slot-accounting --archive-gate`

## Future Work

None.
