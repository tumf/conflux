---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/tui/key_handlers.rs
  - src/tui/state/selection_logic.rs
  - src/tui/command_handlers.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/operator_coordinator.rs
  - src/orchestration/mark_reconciliation.rs
  - src/tui/queue.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/manual_resolve.rs
verifications:
  - id: running-mark-reanalysis-tests
    requirement: Operator execution marks settle into a live current-run queue after a stable interval and trigger dependency analysis without unsafe cancellation
    phase: pre-integration
    owner: conflux-acceptance
    trigger: apply-completion
    automation: Makefile
    evidence: The focused target proves at least one running_mark_reanalysis-prefixed test exists before running all focused unit and scheduler component tests
    rerun: make test-running-mark-reanalysis
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Settle Live-Scheduler Marks into the Current Run

**Change Type**: implementation

## Problem / Context

Before commit `e3167311`, Running-mode Space and bulk `x` immediately mutated the active queue. That commit intentionally separated execution marks from queue intent, but it also removed the only operator path for adding a newly discovered or omitted change to a live run.

The resulting mark-only behavior conflicts with the existing scheduler contract for queue-notification reanalysis and with the active-resolve scenario that expects operator-selected work to enter analysis before resolve completes. The requested replacement is not restoration of the old immediate mutation: it adds a new 10-second mark-set stability period before admission while preserving immediate analysis after a real queue addition.

## Proposed Solution

Add one process-local, event-driven mark stability coordinator at the shared operator/orchestration boundary.

`ExecutionMarkStore` is the concrete notification point because both frontend paths already write there. The TUI `apply_execution_mark` entry point and the API/coordinator `set_execution_mark` and `set_all_execution_marks` entry points notify mark settlement after accepted standalone operator mutations; Space and bulk `x` are not rerouted through the API coordinator. A notification arms one 10-second stability deadline when a live scheduler capable of dynamic queue admission exists. Each later accepted operator notification replaces the pending snapshot and restarts that deadline. System mark revocations, no-ops, refusals, and mark writes performed as part of Start admission do not arm or restart it.

When the deadline expires, settlement reads the current marks and one coherent current reducer/operator view, then adds each marked, loadable, ordinary `not queued` change through the existing admitted queue service. Settlement is additive-only: unmarking affects mark intent but never dequeues current-run work. Active, admitted, explicitly queued, error, retry, resolve, wait, terminal, or otherwise ineligible work remains unchanged. Settlement runs outside reducer and operator mutation locks and acquires the normal application boundary only when applying the final plan.

A real queue addition uses the existing `DynamicQueue` notification path. Once settlement creates a scheduler-local candidate, the existing explicit queue-addition edge starts dependency analysis without another 10-second queue debounce. Analysis may run while resolve is active or ordinary dispatch capacity is zero; apply dispatch remains capacity-gated.

The deadline and pending snapshot are process-local and disposable. A parked persistent scheduler may settle a deadline armed before or after its idle transition. Finite scheduler termination is unchanged, discards an unsettled snapshot, and emits one operator-visible informational outcome identifying that mark settlement was abandoned because the scheduler ended. Restart discards pending stability state, and an idle process with no deadline does not poll.

## Acceptance Criteria

- With a live dynamic-queue scheduler, standalone Space, bulk `x`, and equivalent accepted operator mark mutations do not mutate queue intent during the first 10 seconds after the latest accepted operator mark change.
- Every later accepted operator mark change before settlement restarts the single deadline; system mark reconciliation, refusal, no-op, and Start-admission mark writes do not restart it.
- A deadline armed while the scheduler is Running still settles if the persistent scheduler becomes parked in Select mode, and a standalone mark made while that scheduler is parked may arm settlement.
- At settlement, marked loadable ordinary `not queued` changes enter reducer queue intent and `DynamicQueue` exactly once; duplicate or empty plans are no-ops.
- Settlement is additive-only. Unmarking never dequeues explicit, mark-admitted, active, or waiting work and never emits cancellation or stop.
- Active, error, retry, merge-wait, resolve-wait, terminal, and ordinary ineligible rows retain mark behavior but gain no queue, retry, resolve, stop, or cancellation side effect.
- A rejected Start request cannot later produce partial queue effects from its admission-time mark writes.
- A settled queue addition wakes the scheduler and starts queued-only dependency analysis without another queue debounce, including during active resolve and at zero ordinary capacity.
- Available capacity permits normal dispatch; zero capacity suppresses dispatch without suppressing analysis.
- Processes without a live dynamic-queue scheduler do not arm settlement; finite termination and restart discard unsettled state without changing next-action routing.
- Deterministic tests use paused time, channels, or state transitions rather than short wall-clock correctness thresholds and complete within the repository's default one-second test target.

## Explicit Completion Conditions

The change is complete when repository evidence shows all of the following:

- `ExecutionMarkStore` owns one process-local mark-settlement notifier used by the TUI `apply_execution_mark` and API/coordinator `set_execution_mark` / `set_all_execution_marks` service entry points, with no frontend timer or second durable authority.
- Arming is lock-free; settlement runs on a separate task and routes additions through the existing queue service, reducer transition, hooks, and scheduler notification.
- TUI Space and bulk `x` reach the shared behavior without a frontend timer, new queue key, or second Start action.
- Focused unit tests named `running_mark_reanalysis_*` prove deadline reset, final-snapshot reconciliation, duplicate no-op behavior, source/mode/status exclusions, Start-rejection isolation, persistent-idle settlement, finite-run discard, and restart-empty state.
- Focused lib-target scheduler component tests named `running_mark_reanalysis_*` prove a real `DynamicQueue` addition reaches analysis during active resolve and at zero capacity, with dispatch only after capacity becomes available.
- `make test-running-mark-reanalysis` first fails if no matching tests exist and then passes the focused tests.
- A canonical-vs-promoted scenario-set check proves every pre-change scenario remains after archive except scenarios explicitly replaced by this proposal.

## Scope Rationale

Mark stability and current-run queue/analysis wiring remain one proposal because neither half produces the requested behavior independently: stability without admission remains mark-only, while admission without stability omits the requested settle period.

## Out of Scope

- Adding a dedicated queue key or changing configured Start controls.
- Making execution marks durable across process restart.
- Dequeuing or cancelling work through Space or bulk `x`; `K` remains the change-scoped termination control and explicit dequeue remains the withdrawal control.
- Replacing the scheduler's general queue debounce, analysis-signature, blocked-only, persistent-idle, or finite-termination policy.
- Changing dependency analyzer output or dispatch ordering.
- Treating wait/error marks as implicit retry or resolve commands.

The tracked Rust pre-commit hooks are path-scoped, so proposal-only commit creation does not own Rust validation. Requirement-specific focused tests remain explicit implementation evidence.
