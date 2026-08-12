## Implementation Tasks

- [ ] Add a deterministic compiled-CLI test that forces one `StaleRevision` rejection and recomputation inside a single enqueue invocation. (verification-id: stale-revision-command-audit) (verification: integration - `cargo test --features web-monitoring --test client_cli_tests stale_revision_command_audit` proves injection, same-invocation retry, and exact audit equality)
- [ ] Preserve all production behavior unless the test exposes a real audit defect. (verification-id: stale-revision-command-audit) (verification: integration - `cargo test --features web-monitoring --test client_cli_tests partial_intent` keeps the existing focused paths green)

## Final Validation

Archive validation is authoritative. Expected gate: `cflx openspec validate add-client-command-audit-stale-revision-test --archive-gate`.
