## Implementation Tasks

- [ ] Record each command after actual submission and before settlement interpretation, without recording skipped or pre-submission-rejected commands. (verification-id: client-command-audit) (verification: integration - `cargo test --features web-monitoring --test client_cli_tests partial_intent` compares envelope audit detail with the production command spy for new-mark and pre-marked paths)
- [ ] Preserve existing enqueue routing and partial-intent behavior while correcting only audit accounting. (verification-id: client-command-audit) (verification: integration - `cargo test --features web-monitoring --test client_cli_tests enqueue` exercises existing admission, conflict, stale-revision, and failure paths)

## Final Validation

Archive validation is authoritative. Expected gate: `cflx openspec validate fix-client-command-audit --archive-gate`.
