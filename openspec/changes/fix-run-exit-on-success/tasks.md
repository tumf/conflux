## Implementation Tasks

- [ ] Update run-mode success handling in `src/main.rs` so a successful orchestration result exits instead of polling for restart/stop (verification: `src/main.rs` no longer waits in the success branch after logging completion).
- [ ] Ensure run-scoped background tasks started for signal monitoring and `--web` are shut down on successful completion (verification: lifecycle cleanup is explicit in `src/main.rs` and/or `src/web/mod.rs`).
- [ ] Add regression coverage for successful non-web run completion (verification: automated test exercises a success path and asserts the run command returns without external termination).
- [ ] Add regression coverage for successful `--web` run completion (verification: automated test covers run-mode web monitoring lifecycle and asserts completion does not hang).

## Future Work

- Evaluate whether run-mode error paths should also stop waiting for web retry commands and return immediately in fully non-interactive contexts.
