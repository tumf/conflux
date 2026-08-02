## Implementation Tasks

- [ ] Extend the coherent v2 state/change DTOs with execution mark, queue intent, NEW attention, blocker kind/detail, change-local error detail, action eligibility/reasons, parallel eligibility, timing, latest activity, and worktree relation while preserving process-local/non-durable semantics. (verification: unit - `cargo test --features web-monitoring remote_control_api` verifies DTO serialization and restart-default tests prove every field and ephemeral reset behavior; verification-id: authoritative-snapshot-tests)

- [ ] Wire reducer, operator intent store, log projection, and worktree observation into one snapshot revision without deriving values from display strings or frontend state. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies projection fixtures prove coherent blocked/stalled/error/active/final/worktree-linked snapshots and no-op revision behavior; verification-id: authoritative-snapshot-tests)

- [ ] Publish ordered state updates whenever an in-scope decision field changes and restore the complete state after replay gaps or process-incarnation changes. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies replay/gap tests replace the snapshot and recover marks, queue intent, eligibility, details, timing, activity, and relations without replay inference; verification-id: authoritative-snapshot-tests)

- [ ] Add API and OpenAPI schema coverage for absent values, sanitization, secret/path redaction, mutation readback, and all canonical status/blocker/action combinations. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies authenticated state/change route tests and schema assertions fail on omitted or leaked fields; verification-id: authoritative-snapshot-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate expose-authoritative-operator-snapshot --archive-gate`

The implementation must also pass `cargo test --features web-monitoring remote_control_api`.

## Future Work

- Browser presentation and interaction remain in the consuming Web project or `modernize-web-monitoring-ui`.
