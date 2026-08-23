## Implementation Tasks

- [ ] Remove the settled terminal-error retry block from shared operator-command admission while preserving the requirement for explicit retry intent (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)
- [ ] Align Start, individual retry, and bulk retry so a retained Apply-limit diagnostic does not prevent a fresh explicit execution boundary (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)
- [ ] Align TUI and `/api/v2` action eligibility with shared command admission and keep iteration-limit evidence observational (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)
- [ ] Add regressions proving no automatic redispatch occurs and one explicit retry receives fresh Apply budget while a persistent scheduler remains alive (verification: unit - cargo test operator_command --lib; verification-id: explicit-retry-tests)

## Future Work

The separate `/Users/tumf/bin/claude-auto` empty-account-array failure should be fixed in its owning local wrapper. This change only restores Conflux recovery after any Apply-limit failure.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retire-apply-limit-retry-gate --archive-gate`.
