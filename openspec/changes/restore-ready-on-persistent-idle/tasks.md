## Implementation Tasks

- [ ] Add and exhaustively classify a typed persistent-scheduler idle event in `src/events.rs`, including stable `/api/v2` event vocabulary and semantic lifecycle `idle` mapping. Completion requires event ownership tests to fail if the new variant is omitted from reducer/frontend/remote classification and to prove the event carries no change ID or completion claim. (verification: unit - `cargo test --lib persistent_idle_lifecycle_is_idle -- --list | grep -q persistent_idle_lifecycle_is_idle && cargo test --lib persistent_idle_lifecycle_is_idle`; verification-id: persistent-idle-ready-regressions)

- [ ] Emit the event from `src/parallel/orchestration.rs` immediately before coherent persistent idle parking, using the existing idle predicate rather than a duplicate drain calculation. Add an idle-episode latch whose completion evidence proves fully-drained and blocked/waiting-only inputs emit once, repeated evaluation and no-op notifications emit nothing further, the same scheduler remains alive, and admitted work rearms the next idle edge. (verification: unit - `cargo test --lib persistent_idle_event_is_edge_triggered -- --list | grep -q persistent_idle_event_is_edge_triggered && cargo test --lib persistent_idle_event_is_edge_triggered`; verification-id: persistent-idle-ready-regressions)

- [ ] Project the idle event in TUI and Web as a guarded Running-to-Ready/`select` transition without calling completion helpers or rewriting reducer-derived rows, queue intent, blocker metadata, worktree facts, diagnostics, timing evidence, or execution marks. Completion requires tests for fully drained and blocked/stalled/waiting-only rows, no success message, retained Error/Stopping/Stopped modes, and unchanged non-mode snapshot fields. (verification: integration - `cargo test --lib persistent_idle_projects_ready_without_completion -- --list | grep -q persistent_idle_projects_ready_without_completion && cargo test --lib persistent_idle_projects_ready_without_completion`; verification-id: persistent-idle-ready-regressions)

- [ ] Make existing typed admitted-work start events restore TUI and Web Running after Ready, covering ordinary `WorkspacePreparationStarted` and scheduler-owned resolve/rejection/base-lane work while leaving queue notifications, `AnalysisStarted`, and refresh events Ready. Completion requires a parity regression that traverses idle, a no-op wake, actual admitted work, and a second idle edge. (verification: integration - `cargo test --lib admitted_work_restores_running_after_idle -- --list | grep -q admitted_work_restores_running_after_idle && cargo test --lib admitted_work_restores_running_after_idle`; verification-id: persistent-idle-ready-regressions)

- [ ] Verify `/api/v2` consumes the same idle dispatch and publishes `app_mode: select` at that event's revision, with one event/revision for the first edge and no revision churn for repeated/no-op idle delivery. Completion requires the test to use the authoritative dispatch boundary and shared Web state rather than mutating a projected snapshot directly. (verification: integration - `cargo test --lib persistent_idle_projects_api_ready_once -- --list | grep -q persistent_idle_projects_api_ready_once && cargo test --lib persistent_idle_projects_api_ready_once`; verification-id: persistent-idle-ready-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate restore-ready-on-persistent-idle --archive-gate`.

## Future Work

- None. Scheduler lifetime, queue admission, and retry policy remain intentionally outside this presentation correction.
