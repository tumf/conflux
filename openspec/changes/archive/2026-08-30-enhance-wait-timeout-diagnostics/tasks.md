## Implementation Tasks

- [x] Retain the latest coherent target change and matching execution observation inside `cflx client wait`, replacing it only after a newer coherent observation completes (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: wait-timeout-diagnostics-tests)
- [x] Emit `timeout_ms`, `wait_elapsed_ms`, stable `timeout_stage`, `commands_submitted: 0`, and the target-only `last_observation` from every positive-timeout exit without reading after expiry (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: wait-timeout-diagnostics-tests)
- [x] Add regression coverage for initial-observation timeout, observed timeout, observation replacement, certification/remote stages, target isolation, and preservation of deadline outcome over late inner errors (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: wait-timeout-diagnostics-tests)
- [x] Update CLI documentation for the enriched timeout envelope and clarify that timeout remains observation-only and non-terminal for the proposal (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: wait-timeout-diagnostics-tests)

## Notes

- `Verdict::DeadlineExpired` now carries a `CertificationStage`, so the client can report `repository_certification` and `remote_verification` without parsing the oracle's human-readable text.
- The retained observation is recorded from every coherent observation, before the evaluation that may start certification, so an expiry inside certification reports the observation certification began from rather than a later one.
- Local certification stalling is exercised with a `git` shim on `PATH` that forwards everything except the archive-tree listing, so the stage is decided by where the deadline landed rather than by machine speed.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate enhance-wait-timeout-diagnostics --archive-gate`
