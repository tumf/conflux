## Implementation Tasks

- [ ] Project an accepted persistent-idle Start outcome as Running in the shared Core mode, TUI state, and Web state from the existing authoritative `OperatorCommandApplied::RunDispatched` dispatch. Apply the transition only when the command committed one or more targets against the live persistent-idle episode; keep raw key handling, rejected/no-op Start, and generic scheduler notification free of optimistic mode changes. Completion requires a parity test that fails if Core, TUI, Web, admitted row queue status, or command result revision disagree. (verification: integration - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [ ] Preserve scheduler identity and retry semantics while changing presentation: ordinary Start and explicit-retry Start from persistent-idle Ready must notify the existing scheduler, never spawn a second task, and immediately project Running only after accepted queue/retry intent is committed. Completion requires focused tests for ordinary targets, retry targets, no marked targets, ineligible targets, and a stale idle presentation over an exited scheduler. (verification: integration - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [ ] Rearm the persistent scheduler's idle-edge latch when its coherent reconciliation pass observes queue additions or consumes an accepted explicit-retry edge, not for a bare notification. Completion requires event-order tests proving accepted Start followed by no admitted execution emits exactly one new `PersistentSchedulerIdle` and restores Ready, while duplicate/generic wakeups without queue additions produce neither Running flicker nor duplicate idle edges. (verification: unit - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [ ] Update external lifecycle and `/api/v2` projection coverage so the accepted Start revision reports Running/`working` consistently while execution observation remains truthful: Start acceptance, queue intent, marks, and `app_mode` alone must not set `has_active_work` or invent a current phase; typed dependency-analysis or lifecycle events remain the active-work authority. Completion requires tests covering the accepted-result revision, replayed snapshot, lifecycle deduplication, and the interval before and after `AnalysisStarted`. (verification: integration - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [ ] Add a discovery-guarded `test-idle-start-running` Make target that fails when its focused test filter matches no tests and runs the complete deterministic regression set under the default fast-test policy. Keep repository-wide Rust formatting and clippy in their existing path-scoped commit hooks rather than duplicating them as proposal tasks. (verification: unit - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate show-running-on-idle-start --archive-gate`.

## Future Work

- Dependency-analysis performance tuning remains separate; this change only makes accepted Start feedback immediate and truthful.
