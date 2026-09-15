## Implementation Tasks

- [x] Transfer the owned execution permit from workspace dispatch through background merge settlement, with no release between archive completion and merge task admission. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [x] Make scheduler capacity membership represent each admitted change exactly once through apply, acceptance, archive, merge, automatic resolve, and manual resolve. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [x] Retain lifecycle occupancy in `merge_wait`, then release it only after merged, terminal error/rejection, explicit dequeue, or configured push/publication terminal settlement. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [x] Preserve capacity recovery wakeups when terminal settlement releases a slot, without polling or sticky-trigger replay. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [x] Reconstruct lifecycle occupancy after restart from workspace, Git, and reducer evidence without durable out-of-worktree slot state. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)
- [x] Add `parallel::tests::lifecycle_slot_ownership` regression coverage for archive-to-merge transfer, auto resolve, full-capacity manual resolve, merge wait, terminal release, and TUI/API count parity. (verification: unit - `cargo test --lib parallel::tests::lifecycle_slot_ownership`; verification-id: lifecycle-slot-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate fix-lifecycle-slot-accounting --archive-gate`

## Notes

- Implementation: `src/parallel/lifecycle_slots.rs` owns the one permit source and the
  change-ID-keyed membership. `dispatch_change_to_workspace` records the acquired permit as
  the change's slot instead of moving it into the spawned task;
  `handle_workspace_completion` transfers it to the merge phase (or releases on terminal
  error / rejection / nothing-to-merge); `handle_merge_result_with_tx` releases on merged and
  retains on every other change-local outcome; `reconcile_retained_lifecycle_slots` runs once
  per scheduler pass over the evaluation's coherent reducer view to reconstruct retained
  occupancy after a restart and to release it on terminal settlement.
- `calculate_available_slots` is now `max_concurrent_workspaces` minus the unique union of
  in-flight membership and lifecycle membership. The manual/automatic resolve counters no
  longer subtract capacity — a resolve runs inside the slot its change already owns — and
  remain observability plus base-lane serialization inputs.
- Retained occupancy that cannot reserve a permit fails closed: dispatch capacity is reported
  as zero until occupancy is provable.
- Constitution law 1: slot ownership is in-memory for one process lifetime only. No durable
  lease is written, and restart recovery derives occupancy from reducer/workspace evidence.
- evidence: `cargo test --lib parallel::tests::lifecycle_slot_ownership` — 14 passed, 0 failed (0.01s)
- evidence: full default lib suite — 4430 passed, 0 failed, 18 ignored
- evidence: `cargo clippy --all-targets --all-features` — no warnings
- evidence: `cargo fmt --check` — clean
- evidence: `cflx openspec validate fix-lifecycle-slot-accounting --strict` and `--archive-gate` — both pass

## Future Work

None.
