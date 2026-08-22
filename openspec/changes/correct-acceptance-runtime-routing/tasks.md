## Implementation Tasks

- [ ] Move dedicated Acceptance configuration semantics into validated configuration types and `CommandQueueConfig`, preserving standard load precedence and generated examples (verification: unit - `cargo test config:: --lib`; verification-id: acceptance-runtime-config-tests)
- [ ] Resolve the effective runtime inside the common runner from `operation_type`, including common=30, common=0, and cleanup-review boundaries without changing the runner API (verification: unit - `cargo test config:: --lib`; verification-id: acceptance-runtime-config-tests)
- [ ] Consume Acceptance runtime termination in the executor as a dedicated typed terminal outcome after cleanup proof (verification: unit - `cargo test parallel:: --lib`; verification-id: acceptance-runtime-routing-tests)
- [ ] Route that outcome in dispatch without command retry, missing-verdict retry, Acceptance retry, counter increment, or Apply re-entry; use injected time in tests (verification: unit - `cargo test parallel:: --lib`; verification-id: acceptance-runtime-routing-tests)

## Future Work

Tune the default only from observed Acceptance duration evidence. A durable cross-restart budget requires a separate constitutional design.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate correct-acceptance-runtime-routing --archive-gate`.
