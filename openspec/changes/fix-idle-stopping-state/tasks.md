## Implementation Tasks

- [ ] Add cross-adapter regression coverage for a persistent-idle scheduler whose graceful stop has no remaining work, proving Core, TUI, and Web converge from `Stopping` to inactive `Stopped`/Ready without synthetic work (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)
- [ ] Fix the shared stop settlement and idle-episode projection so no-work completion cannot retain `Stopping`, while active-work graceful stop behavior remains unchanged (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)
- [ ] Add regression coverage for F5/cancel-stop from an idle-origin stop, proving it restores Ready and only returns to `Running` after accepted Start or typed work-start evidence (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-idle-stopping-state --archive-gate`.
