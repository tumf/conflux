## Implementation Tasks

- [ ] Task 1: Bound structured missing-candidate diagnostics in `src/parallel/queue_state.rs`. (verification: unit - add or update a focused test in `src/parallel/tests/executor.rs` or adjacent queue-state tests that invokes missing-candidate reconciliation repeatedly and asserts the first `candidate_not_found` observation remains visible while repeated WARN/TUI-visible emissions are bounded; completion condition: the test fails against unconditional `warn!` emission and passes after the bounded path is used)
- [ ] Task 2: Preserve queue reconciliation behavior for missing and loadable candidates. (verification: integration - run `cargo test scheduler_reconciliation` plus any new missing-candidate regression test, covering that missing candidates are not pushed into scheduler-local `queued` and loadable reducer-queued changes still are; completion condition: test output shows all targeted scheduler reconciliation tests pass)
- [ ] Task 3: Confirm observability-only suppression state. (verification: manual - inspect `src/parallel/queue_state.rs` and `src/parallel/tests/executor.rs` or adjacent tests to confirm diagnostic dedupe/rate-limit state is in-memory only and does not affect queue insertion, resume routing, archive, merge, or next-action decisions; completion condition: code review evidence identifies the suppression state and its limited use)
- [ ] Task 4: Run repository formatting and targeted Rust verification for the touched modules. (verification: manual - run `cargo fmt --check` and `cargo test scheduler_reconciliation` or the narrower test filters added for this change; completion condition: commands complete successfully or any heavy tests are marked according to repository policy)

## Future Work

- Broader log-mining UX improvements are intentionally separate from this fix.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate fix-missing-candidate-log-spam --archive-gate`
