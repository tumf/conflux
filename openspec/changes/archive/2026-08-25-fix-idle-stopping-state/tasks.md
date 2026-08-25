## Implementation Tasks

- [x] Add a failing-before-fix cross-adapter regression that drives the real scheduler through `AllCompleted`/park into the no-work graceful-stop boundary, rather than injecting a synthetic `Stopped` event, and proves Core, TUI, and Web converge from `Stopping` to inactive `Stopped`/Ready without synthetic work (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)
- [x] Fix the shared stop settlement and idle-episode projection so no-work completion cannot retain `Stopping`, while active-work graceful stop behavior remains unchanged (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)
- [x] Add a failing-before-fix regression for F5/cancel-stop after `AllCompleted` leaves a live parked scheduler without the idle-episode fact, proving the shared boundary restores Ready and only returns to `Running` after accepted Start or typed work-start evidence (verification: integration - `cargo test idle_start_running_tests --locked`; verification-id: idle-stopping-regressions)

## Notes

The fix has two halves, because the two reported sequences fail at two different
places and one of them is not reachable from a projection at all.

- Idle-episode projection: `crate::events::graceful_stop_is_idle_origin` is the
  one shared rule Core (`CoreMode::apply_event`), the TUI
  (`AppState::handle_stopping`), and Web (`WebState::apply_dispatch`) route
  `Stopping` through. Run control admits a graceful stop from Ready only over a
  live parked scheduler, so a `Stopping` transition observed in Ready is exactly
  the idle-origin case — including the Ready an `AllCompleted` settlement leaves,
  which owns no typed idle edge and is where cancel-stop used to claim Running.
- Stop settlement: the persistent scheduler now reads the run owner's
  graceful-stop request (`ParallelExecutor::graceful_stop`, bound through
  `ParallelRunService` and `run_orchestrator_parallel` from the same
  `TuiRunSupervisor` flag shared run control writes). When it would park with an
  accepted stop pending and `is_fully_drained` over its own coherent reducer view,
  that evaluation *is* the boundary the stop asked for, so the run ends and
  publishes its one terminal `Stopped`. Before this the flag had no reader at all:
  the scheduler parked in a wait with no timer and nothing left to wake it, which
  is why `Stopping` was retained indefinitely. A blocked-only park, a resolve or
  reject wait, and a pending merge all still hold work, so they reach their
  existing boundary unchanged, and a run with no pending request never settles.

Two pre-existing tests arranged states the process cannot be in and were
corrected rather than accommodated: `convergence_tests` published `running` to
Core and the TUI while leaving Web in `select`, and the Web event-ownership case
arranged a graceful stop over a frontend that had never run.

`proposal.md` gained the `## Retired Scenarios` declaration the promotion-safety
regression requires for the two scenarios this change's delta replaces. It was
missing before implementation started, and
`openspec_cmd::promotion::tests::every_pending_change_promotes_without_dropping_a_scenario`
failed on it.

- evidence: `cargo test --lib` — 4121 passed, 0 failed, 18 ignored
- evidence: `cargo test --lib idle_stopping` fails both new regressions when the
  two fixes are neutralized, and passes with them
- evidence: `cargo fmt --all` clean; `cargo clippy --all-targets --all-features`
  reports no warnings

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-idle-stopping-state --archive-gate`.
