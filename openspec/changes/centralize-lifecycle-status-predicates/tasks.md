## Implementation Tasks

- [ ] Add shared lifecycle status predicate functions beside the canonical status constants and replace local status lists. (verification: integration - `cargo test --lib web::remote_control_api::tests::operator_snapshot_tests && cargo test --lib web::remote_control_api::tests::projection_tests && cargo test --lib tui::render`; verification-id: focused-tests)
- [ ] Make both Web summary construction paths use one implementation and add `pushed`/`rejecting` regressions. (verification: integration - `cargo test --lib web::remote_control_api::tests::operator_snapshot_tests && cargo test --lib web::remote_control_api::tests::projection_tests && cargo test --lib tui::render`; verification-id: focused-tests)
- [ ] Make TUI post-archive checks, `push_change_row_hints`, and header active counts consume the same predicates; retain existing labels and leave per-status spinner/badge formatting unchanged. (verification: integration - `cargo test --lib web::remote_control_api::tests::operator_snapshot_tests && cargo test --lib web::remote_control_api::tests::projection_tests && cargo test --lib tui::render`; verification-id: focused-tests)

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate centralize-lifecycle-status-predicates --archive-gate`.

## Future Work

- None.
