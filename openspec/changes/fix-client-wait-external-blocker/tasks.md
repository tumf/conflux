## Implementation Tasks

- [ ] Classify structured external blockers in `src/client/wait.rs` as manual-action wait results while retaining observation for generic and dependency-driven blocked rows. (verification: integration - `cargo test --test client_cli_tests wait_releases_external_blocker_without_waiting_for_operator -- --exact`; verification-id: client-wait-external-blocker)
- [ ] Preserve the existing observation-only envelope and update CLI-facing documentation for the external-blocker exception. (verification: integration - assertions in `tests/client_cli_tests.rs` verify outcome, exit status, detail, zero commands, and unchanged repository; verification-id: client-wait-external-blocker)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-client-wait-external-blocker --archive-gate`.
