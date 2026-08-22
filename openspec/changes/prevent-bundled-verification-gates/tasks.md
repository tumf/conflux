## Implementation Tasks

- [ ] Extend active-task parsing to retain each checkbox's verification ownership marker, concrete command, referenced verification ID, and line number without deriving semantics from free text (verification: unit - `cargo test openspec_cmd --lib`; verification-id: proposal-gate-validation-tests)
- [ ] Add deterministic cohesion validation for reused change-blocking IDs and reject mismatched ownership markers or commands with diagnostics naming the ID and affected lines (verification: unit - `cargo test openspec_cmd --lib`; verification-id: proposal-gate-validation-tests)
- [ ] Add the minimal structural heavyweight-command policy and reject those commands as change-blocking while preserving focused shared commands (verification: unit - `cargo test openspec_cmd --lib`; verification-id: proposal-gate-validation-tests)
- [ ] Update bundled proposal guidance and installation assertions with bounded-proof and operational-observation examples (verification: unit - `cargo test openspec_cmd --lib`; verification-id: proposal-gate-validation-tests)

## Future Work

The heavyweight-command list may be extended only from observed false negatives with deterministic syntax.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate prevent-bundled-verification-gates --archive-gate`.
