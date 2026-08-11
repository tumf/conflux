## Context

Execution marks and queue intent are intentionally separate projections. Before `e3167311`, Running-mode mark controls immediately mutated the current queue; the current mark-only behavior leaves no operator path for adding newly selected work to a live run and contradicts scheduler scenarios for queue admission during resolve.

The scheduler already has the required post-admission behavior: `DynamicQueue` notifications wake reconciliation, genuine candidate additions bypass the ordinary queue debounce, analysis can run at zero capacity, and dispatch remains capacity-gated. The missing policy belongs before queue admission.

## Goals / Non-Goals

### Goals

- Admit operator-marked work to a live current run after one 10-second stable interval.
- Reuse the shared operator queue service and scheduler notification path.
- Keep settlement additive-only and incapable of dequeue, retry, resolve, stop, or cancellation effects.
- Keep timing process-local, lock-safe, and deterministic under test.

### Non-Goals

- Merge execution marks and queue intent into one state axis.
- Restore the old immediate mark-to-queue mutation.
- Cancel or dequeue work from mark controls.
- Add polling, a second scheduler debounce, or durable mark workflow state.

## Decisions

### Decision: debounce standalone operator mark intent before queue mutation

A real standalone operator mark mutation replaces the pending snapshot and restarts one 10-second deadline only while a live scheduler capable of dynamic queue admission exists. System reconciliation, automatic mark revocation, no-op/refused commands, and mark writes performed inside Start admission do not arm or restart the deadline.

Settlement reads current marks rather than trusting a stale captured set. This lets lifecycle-driven mark revocation affect the final plan without extending the deadline or starving settlement.

### Decision: gate on scheduler liveness, not presentation mode

Persistent schedulers report Select while parked. A deadline armed before that transition must still settle, and a standalone mark while parked may wake the same live run. Presentation mode is therefore not the authority; scheduler liveness and dynamic-queue capability are.

A finite scheduler gets no new termination barrier. If it exits before settlement, its pending snapshot is discarded and one operator-visible informational outcome reports that mark settlement was abandoned because the scheduler ended.

### Decision: notify from `ExecutionMarkStore` service entry points and settle on a separate task

`ExecutionMarkStore` is the common storage already written by both paths, but accepted-command context remains at its service entry points. TUI `apply_execution_mark` and API/coordinator `set_execution_mark` and `set_all_execution_marks` notify settlement after accepted standalone operator writes. Space and bulk `x` remain on the TUI service path rather than detouring through the API coordinator.

Mark outcomes can occur while the operator mutation guard and reducer lock are held, including system revocations inside event dispatch. Notification records/replaces timer state without re-entering the application transaction. The timer task later acquires the normal operator guard and reducer view in the established order, reclassifies current state, and applies additions. Frontends own no timers.

### Decision: retain mark and queue as distinct authorities

Marks remain process-local target intent. Queue membership remains reducer-owned workflow intent and changes only through the existing queue command service. The coordinator is a policy bridge, not a projection merger.

### Decision: settlement is additive-only

Stable settlement may add marked eligible ordinary `not queued` work. It never removes queue membership. Unmarking changes only mark intent; explicit dequeue remains the withdrawal operation.

This removes provenance state, restart asymmetry, and accidental withdrawal of active or explicitly queued work.

### Decision: final eligibility is checked at settlement

At deadline expiry, classification uses one coherent current reducer/operator view. Active, admitted, queued, terminal, waiting, errored, retry-scoped, resolve-scoped, unavailable, or otherwise ineligible rows are skipped. The queue service must not be called on terminal-error rows because its explicit behavior may create a retry edge.

Start-admission mark writes are excluded from arming. A rejected Start therefore cannot produce delayed partial queue effects.

### Decision: timer state is disposable

The pending snapshot and deadline are in-memory only. Restart, finite scheduler exit, or scheduler replacement discards them. They do not influence resume routing, acceptance, archive, merge, or next-action decisions.

## Event Flow

```text
accepted standalone operator mark outcome
        |
        +-- no live dynamic-queue scheduler --> mark only
        |
        v
replace pending snapshot metadata
restart 10-second deadline
        |
        v
separate settlement task
        |
        v
acquire operator guard, then coherent reducer view
read current marks and classify current lifecycle state
        |
        +-- eligible marked not-queued --> shared AddToQueue
        |                                  reducer + DynamicQueue + notify
        |
        +-- unmarked/active/wait/error/terminal/ineligible --> no queue side effect

DynamicQueue notification
        |
        v
scheduler ingestion/reconciliation
        |
        v
genuine candidate-addition reanalysis edge
        |
        +-- capacity > 0 --> normal dispatch
        +-- capacity = 0 --> analysis only; dispatch waits
```

## Risks / Trade-offs

- A fixed 10-second interval delays intentional single-mark admission. It is retained because the requested stability policy avoids queue churn; configurability is future work. It currently equals the scheduler queue debounce duration by coincidence, but is an independent pre-admission interval; the existing queue-notification bypass prevents the two intervals from stacking.
- Bulk and single outcomes can race deadline expiry. The operator transaction gives settlement and later commands one total order; tests exercise both orders deterministically.
- A finite run may end before settlement. Preserving its existing termination contract is safer than introducing a hidden liveness barrier for a frontend capability it does not normally expose.

## Migration Plan

- Promote complete canonical requirements without dropping unrelated paragraphs or scenarios.
- Add the coordinator and focused tests without changing existing queue/scheduler contracts.
- Reuse current `DynamicQueue`, reducer, queue hook, and reanalysis paths.
- Replace only assertions that every live-scheduler mark is permanently queue-inert; retain non-live, status, Start, and cancellation safety coverage.

## Open Questions

None. The interval is fixed at 10 seconds, settlement is additive-only, and active cancellation remains on `K`.
