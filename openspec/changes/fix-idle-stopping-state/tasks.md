## Implementation Tasks

- [ ] Add a failing-before-fix cross-adapter regression that drives the real scheduler through `AllCompleted`/park into the no-work graceful-stop boundary, rather than injecting a synthetic `Stopped` event, and proves Core, TUI, and Web converge from `Stopping` to inactive `Stopped`/Ready without synthetic work (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)
- [ ] Fix the shared stop settlement and idle-episode projection so no-work completion cannot retain `Stopping`, while active-work graceful stop behavior remains unchanged (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)
- [ ] Add a failing-before-fix regression for F5/cancel-stop after `AllCompleted` leaves a live parked scheduler without the idle-episode fact, proving the shared boundary restores Ready and only returns to `Running` after accepted Start or typed work-start evidence (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-idle-stopping-state --archive-gate`.
