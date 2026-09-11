## Implementation Tasks

- [ ] Add `support.rs` and move the shared `TestWorkspaceManager`, builders, Git initialization, config, and failure helpers. (verification: integration - `cargo test --lib parallel::tests && cargo test --lib parallel_run_service`; verification-id: focused-tests)
- [ ] Repoint sibling test modules and remove only byte-equivalent duplicate helpers. Keep non-identical local variants, including helpers in `src/parallel_run_service.rs`. (verification: integration - `cargo test --lib parallel::tests && cargo test --lib parallel_run_service`; verification-id: focused-tests)
- [ ] Verify test inventory and behavior remain unchanged; do not split behavioral test groups or optimize runtime in this change. (verification: integration - `cargo test --lib parallel::tests && cargo test --lib parallel_run_service`; verification-id: focused-tests)

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate extract-parallel-test-support --archive-gate`.

## Future Work

- None.
