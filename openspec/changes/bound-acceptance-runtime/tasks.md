## Implementation Tasks

- [x] Add validated `acceptance_max_runtime_secs` configuration with a 1800-second default, 60..=10800 range, zero rejection, and normal precedence (verification: unit - `cargo test config:: --lib`; verification-id: acceptance-runtime-config-tests)
- [x] Route Acceptance command construction through the Acceptance-specific limit without changing other command classes (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: acceptance-runtime-tests)
- [x] Convert Acceptance runtime expiry into a typed non-retryable failure after bounded owned-process-group cleanup, with actionable limit and cleanup diagnostics (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: acceptance-runtime-tests)
- [x] Add regression coverage for common-limit interaction and retry classification, and update generated config/operator documentation for the new guard (verification: unit - `cargo test orchestration::acceptance --lib` and `cargo test config:: --lib`; verification-id: acceptance-runtime-tests; verification-id: acceptance-runtime-config-tests)

## Future Work

Per-verification runtime budgets may be proposed after the declaration model proves stable.

## Notes

- evidence: `cargo test orchestration::acceptance --lib` — 67 passed, 0 failed (10 new `runtime_limit` tests).
- evidence: `cargo test config:: --lib` — 177 passed, 0 failed (5 new `acceptance_max_runtime_*` / effective-limit tests).
- evidence: `cargo test --lib` — 4031 passed, 0 failed, 17 ignored.
- evidence: `cargo clippy --all-targets --all-features -- -D warnings` clean; `cargo fmt --all -- --check` clean.
- evidence: `cargo test --features heavy-tests --test process_cleanup_test absolute_runtime_limit` — 4 passed, including the new `acceptance_stays_bounded_when_the_common_limit_is_disabled` real-process expiry test. Integration evidence (real OS process groups), not unit evidence; it supplements the unit-scoped decision tests rather than standing in for them.
- Unit-scoped decision logic lives in `src/orchestration/acceptance.rs` (`acceptance_runtime_limit_secs`, `classify_acceptance_runtime_limit`, `AcceptanceRuntimeLimit`, `AcceptanceResult::permits_acceptance_retry`) and in `src/config/types.rs` (`get_acceptance_runtime_limit_secs`, `validate_acceptance_max_runtime_secs`); both are pure over their inputs and use no real process, filesystem, or clock.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate bound-acceptance-runtime --archive-gate`.
