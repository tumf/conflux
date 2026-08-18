## Implementation Tasks

- [ ] Replace the live-owner ordinary `Route::Queue` branch in `src/client/enqueue.rs` with target-scoped additive execution marking through the existing shared operator-command path; preserve retry, already-admitted, unsafe-target, stale-revision, and owner-incarnation routing. (verification: unit - `cargo test --lib client::enqueue::tests` proves no ordinary live-owner route constructs `SetQueueIntent`, the requested mark is added, and unrelated marks remain unchanged; verification-id: client-enqueue-analysis-tests)
- [ ] Connect ordinary CLI/MCP enqueue completion to the existing mark-settlement/analyze admission result without adding a client-owned timer, queue mutation, or analyze implementation; return `admitted` only from authoritative queued/active execution evidence and return bounded typed non-success for mark-only or interrupted outcomes. (verification: integration - `tests/client_cli_tests.rs` drives a command-capable owner through mark settlement to analyze-driven admission and proves mark settlement alone is not reported as admission; verification-id: client-enqueue-analysis-tests)
- [ ] Update CLI/MCP contract text and shared adapter assertions so both surfaces describe and use the same mark-first analyze path while keeping execution marks and queue intent distinct. (verification: integration - `cargo test --test client_cli_tests cli_surface && cargo test --test client_cli_tests enqueue` exercises client CLI/MCP help, tool-schema assertions, and the shared enqueue path to prove no adapter-specific admission route; verification-id: client-enqueue-analysis-tests)
- [ ] Preserve the existing safety matrix with focused regressions for retryable evidence, already-admitted idempotence, unrelated pre-existing marks, stale revisions, owner replacement, unsafe targets, and execution ID resolution. (verification: integration - `cargo test --test client_cli_tests enqueue`; verification-id: client-enqueue-analysis-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate route-client-enqueue-through-mark-analysis --archive-gate`

## Future Work

- None.
