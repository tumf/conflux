## Implementation Tasks

- [ ] Add validated `acceptance_max_runtime_secs` configuration with a 1800-second default, 60..=10800 range, zero rejection, and normal precedence (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: acceptance-runtime-tests)
- [ ] Route Acceptance command construction through the Acceptance-specific limit without changing other command classes (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: acceptance-runtime-tests)
- [ ] Convert Acceptance runtime expiry into a typed non-retryable failure after bounded owned-process-group cleanup, with actionable limit and cleanup diagnostics (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: acceptance-runtime-tests)
- [ ] Add regression coverage and update generated config/operator documentation for the new guard (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: acceptance-runtime-tests)

## Future Work

Per-verification runtime budgets may be proposed after the declaration model proves stable.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate bound-acceptance-runtime --archive-gate`.
