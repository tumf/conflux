## Implementation Tasks

- [x] Add shared lifecycle status predicate functions beside the canonical status constants and replace local status lists. (verification: integration - `cargo test --lib web::remote_control_api::tests::operator_snapshot_tests && cargo test --lib web::remote_control_api::tests::projection_tests && cargo test --lib tui::render`; verification-id: focused-tests)
- [x] Make both Web summary construction paths use one implementation and add `pushed`/`rejecting` regressions. (verification: integration - `cargo test --lib web::remote_control_api::tests::operator_snapshot_tests && cargo test --lib web::remote_control_api::tests::projection_tests && cargo test --lib tui::render`; verification-id: focused-tests)
- [x] Make TUI post-archive checks, `push_change_row_hints`, and header active counts consume the same predicates; retain existing labels and leave per-status spinner/badge formatting unchanged. (verification: integration - `cargo test --lib web::remote_control_api::tests::operator_snapshot_tests && cargo test --lib web::remote_control_api::tests::projection_tests && cargo test --lib tui::render`; verification-id: focused-tests)

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate centralize-lifecycle-status-predicates --archive-gate`.

## Notes

- Two predicates now own the vocabulary in `src/orchestration/operator_command.rs`: the new `is_completed_status` (backed by `COMPLETED_STATUSES = {archived, merged, pushed}`) and the pre-existing `is_active_status` (backed by `ACTIVE_STATUSES`), which the proposal names as the in-progress set verbatim. No second in-progress spelling was introduced.
- `OrchestratorStateSnapshot::from_changes_with_shared_state` no longer computes totals at all; it builds the rows and delegates to `refresh_summary`, so the two paths cannot diverge again.
- Behavior corrected as specified: `pushed` now counts as completed on the refresh path, and `rejecting` now counts as in progress on the refresh path, gains the `K: kill` hint, and is counted by the TUI header. Per-status spinner and badge formatting is untouched.
- evidence: `cargo test --lib web::remote_control_api::tests::operator_snapshot_tests` — 35 passed, 0 failed (includes the new `summary_totals_classify_pushed_and_rejecting_the_same_in_both_construction_paths`)
- evidence: `cargo test --lib web::remote_control_api::tests::projection_tests` — 15 passed, 0 failed
- evidence: `cargo test --lib tui::render` — 191 passed, 0 failed (includes the new `running_header_counts_every_shared_active_status` / `running_header_counts_no_inactive_status`)
- evidence: `cargo test --lib web::state` — 38 passed, 0 failed (includes the new `summary_totals_classify_every_canonical_status`)
- evidence: full `cargo test --lib` — 4374 passed, 0 failed, 18 ignored
- evidence: `cargo clippy --all-targets --all-features` clean; `cargo fmt --check` clean

## Future Work

- None.
