## Implementation Tasks

- [ ] Retain the latest coherent target change and matching execution observation inside `cflx client wait`, replacing it only after a newer coherent observation completes (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: wait-timeout-diagnostics-tests)
- [ ] Emit `timeout_ms`, `wait_elapsed_ms`, stable `timeout_stage`, `commands_submitted: 0`, and the target-only `last_observation` from every positive-timeout exit without reading after expiry (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: wait-timeout-diagnostics-tests)
- [ ] Add regression coverage for initial-observation timeout, observed timeout, observation replacement, certification/remote stages, target isolation, and preservation of deadline outcome over late inner errors (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: wait-timeout-diagnostics-tests)
- [ ] Update CLI documentation for the enriched timeout envelope and clarify that timeout remains observation-only and non-terminal for the proposal (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: wait-timeout-diagnostics-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate enhance-wait-timeout-diagnostics --archive-gate`
