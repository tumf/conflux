## Implementation Tasks

- [ ] Make completion-trigger consumption explicit in the active scheduler loop so one actual resolve/workspace/merge completion permits one immediate `ResolveCompletion` evaluation, while a later timer-only wake uses ordinary debounce behavior; completion requires the event reason to be reset without changing queue-notification, repair-candidate, or slot-recovery semantics. (verification: unit - `cargo test parallel::tests --lib`; verification-id: scheduler-local-tests)
- [ ] Add a scheduler regression test that drives an actual completion reason through zero-capacity analysis followed by multiple timer-equivalent iterations and asserts the analyzer is invoked only once until a new qualifying event occurs; completion requires the test to fail against the pre-fix reason-retention behavior. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-local-tests)
- [ ] Add or extend coverage proving a second real completion event can trigger another immediate analysis and that capacity recovery dispatches eligible queued work without queue notification or user action. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-local-tests)
- [ ] Preserve zero-capacity and explicit-bypass contracts by running the existing manual-resolve, auto-resolve, resolve-completion dispatch, queue-notification, repair-candidate, and slot-recovery tests; completion requires no expectation to be weakened merely to pass the new regression. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-local-tests)
- [ ] Run Rust formatting, linting, and default-path tests; completion requires `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` to succeed, with any test over one second optimized or marked heavy according to repository policy. (verification: integration - `cargo fmt --check && cargo clippy -- -D warnings && cargo test`; verification-id: scheduler-local-tests)

## Future Work

No external, manual, deployed-service, or long-wait verification is required.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-repeated-resolve-completion-analysis --archive-gate`
