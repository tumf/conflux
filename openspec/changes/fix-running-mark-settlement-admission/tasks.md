## Implementation Tasks

- [ ] Add a production-equivalent regression that keeps one change active, marks another ordinary eligible target through the shared operator/API path, advances the stability window, and proves queue intent plus exactly one scheduler analysis wake. The test must fail against the pre-fix behavior rather than manually invoking settlement. (verification: integration - `cargo test --locked running_mark_reanalysis`; verification-id: running-mark-settlement-regression)
- [ ] Repair the shared owner/application settlement lifecycle so every accepted changed mark during a live persistent scheduler retains its timer task and runtime binding until the target is reconciled or receives a stable observable exclusion. Do not enqueue in a frontend or synthesize Start. (verification: integration - `cargo test --locked running_mark_reanalysis`; verification-id: running-mark-settlement-regression)
- [ ] Add stable observability for arm and completion failures, and verify TUI Space/bulk and client/API mark adapters converge on the same settlement path without changing unrelated queue intent. (verification: integration - `cargo test --locked running_mark_reanalysis`; verification-id: running-mark-settlement-regression)

## Final Validation

Archive validation is authoritative. Expected gate: `cflx openspec validate fix-running-mark-settlement-admission --archive-gate`.
