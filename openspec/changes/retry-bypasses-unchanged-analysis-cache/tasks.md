## Implementation Tasks

- [ ] Preserve target-specific explicit-retry identity from accepted `retry_change` dispatch through the live scheduler's next eligible analysis evaluation; consume it only after that evaluation, without weakening dependency, capacity, eligibility, or retry-budget guards. (verification: integration - `cargo test --lib retry_change_bypasses_unchanged_analysis_input_and_dispatches`; verification-id: explicit-retry-dispatch-regression)
- [ ] Add a deterministic paused-time regression covering a previously completed matching analysis signature, accepted `retry_change`, one fresh analyzer invocation, actual dispatch start, and suppression on later unchanged timer wakes. (verification: integration - `cargo test --lib retry_change_bypasses_unchanged_analysis_input_and_dispatches`; verification-id: explicit-retry-dispatch-regression)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate retry-bypasses-unchanged-analysis-cache --archive-gate`.
