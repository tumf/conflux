## Implementation Tasks

- [x] Project an accepted persistent-idle Start outcome as Running in the shared Core mode, TUI state, and Web state from the existing authoritative `OperatorCommandApplied::RunDispatched` dispatch. Because the payload distinguishes only `scheduler_started`, gate the transition on ordered projection facts: Select mode, `persistent_scheduler_idle: true`, `scheduler_started: false`, and non-empty committed targets; every publisher using this shape must guarantee it woke the live scheduler. Keep raw key handling, rejected/no-op Start, non-Start queue admission, and generic scheduler notification free of optimistic mode changes. Completion requires a parity test that fails if Core, TUI, Web, admitted row queue status, or command result revision disagree. (verification: integration - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [x] Preserve scheduler identity and retry semantics while changing presentation: ordinary Start and explicit-retry Start from persistent-idle Ready must notify the existing scheduler, never spawn a second task, and immediately project Running only after accepted queue/retry intent is committed. Completion requires focused tests for ordinary targets, retry targets, no marked targets, ineligible targets, and a stale idle presentation over an exited scheduler. (verification: integration - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [x] Rearm the persistent scheduler's idle-edge latch with a level-based predicate when its coherent reconciliation pass observes at least one queued row or an unconsumed accepted explicit-retry hold, not from an individual reducer outcome or bare notification. Completion requires event-order tests proving accepted Start followed by no admitted execution emits exactly one new `PersistentSchedulerIdle` and restores Ready; duplicate/generic wakes without visible intent emit no edge; and non-Start client enqueue neither projects Running from queue addition nor causes mode churn when a no-work idle edge reaches an already-Select frontend. (verification: unit - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [x] Update external lifecycle, `LifecycleModeMirror`, and `/api/v2` projection coverage so the accepted Start revision reports Running/`working` consistently while execution observation remains truthful: the mirror must absorb accepted idle-Start as Running so a later no-work idle edge returns it to idle; Start acceptance, queue intent, marks, and `app_mode` alone must not set `has_active_work` or invent a current phase; typed dependency-analysis or lifecycle events remain the active-work authority. Completion requires tests covering the accepted-result revision, replayed snapshot, lifecycle deduplication, non-Start enqueue, the interval before and after `AnalysisStarted`, and no-work mirror closure. (verification: integration - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [x] Extend command-order coverage for cancel-stop: idle-origin stop without Start restores Ready, accepted Start followed by stop restores Running, and typed work start winning during Stopping clears the idle fact before cancel-stop restores Running. Preserve graceful/force-stop controls and terminal clearing. (verification: integration - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

- [x] Add a discovery-guarded `test-idle-start-running` Make target that fails when its focused test filter matches no tests and runs the complete deterministic regression set under the default fast-test policy. Keep repository-wide Rust formatting and clippy in their existing path-scoped commit hooks rather than duplicating them as proposal tasks. (verification: unit - `make test-idle-start-running`; verification-id: idle-start-running-regressions)

## Implementation Notes

- The shared gate is `crate::events::accepted_start_opens_idle_run_episode` in `src/events.rs`. `CoreMode::apply_event`, `AppState::handle_operator_command_applied`, `WebState::apply_dispatch`, and `LifecycleModeMirror::absorb` all route the same dispatch through it, so no projection can re-derive the answer differently.
- `RunDispatched` now *means* scheduler-wake evidence for the targets it names. `OperatorOutcome::Retry(plan)`, which holds no scheduler effect, therefore publishes nothing instead of reusing the shape; every production retry reaches the run-control path that does hold it. `run_outcome_event` carries a `debug_assert!` on `SchedulerEffect::dispatched()` so a future no-dispatch path cannot silently teach frontends to claim Running.
- The scheduler rearm is a *level* read against a per-episode baseline (`persistent_idle_baseline`), recorded from the same coherent snapshot the park was decided from. Comparing against the baseline rather than against emptiness is what keeps a blocked-only park quiet while still catching intent an accepted Start committed under a concurrent enqueue.
- Two pre-existing tests asserted the replaced contract and now assert the new one: `cross_adapter_tests::persistent_idle_commands_use_live_scheduler` and `change_error_f5_retry_tests::change_error_f5_retry_persistent_idle_select_is_identical_on_both_adapters`. The latter also gained the Web half of its arrangement (`WebState::set_persistent_scheduler_idle`, test-only), which it had never staged; without it the comparison reported an arrangement gap as an adapter divergence. No-op wake coverage still requires Ready.

## Final Validation

Scenario preservation must pass `make check-scenario-set`. Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate show-running-on-idle-start --archive-gate`.

Results from this Apply invocation:

- `make test-idle-start-running` — passed. Discovery reported 14 focused lib-target tests; all 14 passed in 0.13s. The gate was proven fail-safe with `make test-idle-start-running IDLE_START_FILTER=no_such_test`, which exits 1 without running anything.
- `cargo test` (default fast path) — passed: 3877 lib tests plus every integration target, 0 failed.
- `cargo clippy --all-targets -- -D warnings` — clean; `cargo fmt --all` applied.
- `make check-scenario-set` — passed (7 capability deltas compared against canonical specs).
- `cflx openspec validate show-running-on-idle-start --strict` — passed.
- `cflx openspec validate show-running-on-idle-start --archive-gate` — passed.

## Future Work

- Dependency-analysis performance tuning remains separate; this change only makes accepted Start feedback immediate and truthful.
