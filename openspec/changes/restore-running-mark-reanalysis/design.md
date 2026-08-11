## Context

Execution marks and queue intent are intentionally separate projections, but Running mode previously connected settled operator selection to current-run admission. The current mark-only behavior leaves no command available to add newly marked work to an active run and contradicts scheduler scenarios that expect TUI dynamic queue admission during resolve.

The scheduler already has the required post-admission behavior: `DynamicQueue` notifications wake reconciliation, genuine candidate additions bypass the ordinary queue debounce, analysis can run at zero capacity, and dispatch remains capacity-gated. The missing boundary is therefore before queue admission, not inside dependency analysis.

## Goals / Non-Goals

### Goals

- Restore Running-mode mark-driven current-run admission after a 10-second stable mark interval.
- Reuse the shared operator queue service and scheduler notification path.
- Make unmark removal safe by limiting it to still-pending memberships created by mark reconciliation.
- Keep timing and provenance process-local and deterministic under test.

### Non-Goals

- Merge execution marks and queue intent into one state axis.
- Cancel active work from mark controls.
- Add polling or a second scheduler debounce.
- Persist unsettled operator intent outside the process.

## Decisions

### Decision: debounce mark intent before queue mutation

The 10-second interval applies to the latest Running-mode mark set, not to the scheduler after admission. Each real mark mutation replaces the pending snapshot and restarts one deadline. This matches the requested settle behavior while avoiding repeated queue/hook churn during rapid selection edits.

Once settlement creates a genuine queue candidate, the existing queue-addition edge remains immediate. Adding a second scheduler delay would duplicate policy and conflict with `Parallel Analysis Targeting`.

### Decision: place coordination above frontend adapters

The stability coordinator belongs at the shared operator/orchestration boundary because local TUI and remote operator commands can express the same mark intent. Frontend-local timers would create divergent behavior and could race authoritative outcomes.

The coordinator reacts only to accepted state-changing mark outcomes. Refusals and no-ops do not restart the deadline.

### Decision: retain mark and queue as distinct authorities

Marks remain process-local target intent. Queue membership remains reducer-owned workflow intent and is changed only through the existing queue command service. The coordinator is a policy bridge, not a projection merger.

### Decision: provenance fences unmark removal

The coordinator keeps process-local provenance for queue memberships it successfully created and that remain pending. Stable unmark may request removal only for those IDs. Provenance is discarded when the membership becomes active, terminal, explicitly dequeued, or otherwise leaves the pending ordinary queue state.

This prevents mark controls from withdrawing explicit queue actions or cancelling admitted work. A restart loses provenance, so post-restart unmark is conservative and removes nothing automatically.

### Decision: final eligibility is checked at settlement

The pending snapshot is intent, not admission authority. At settlement, classification uses one coherent current reducer/operator view. Any row that became active, terminal, waiting, errored, retry-scoped, resolve-scoped, or ineligible is skipped without side effects.

### Decision: timer state is disposable

The latest pending mark snapshot, deadline, and provenance are in-memory only. They are discarded on restart and do not influence resume routing, acceptance, archive, or merge decisions. This follows the workspace-local workflow-state constitution.

## Event Flow

```text
accepted Running mark outcome
        |
        v
replace pending mark snapshot
restart 10-second stability deadline
        |
        v
settlement reads coherent current state
        |
        +-- eligible marked not-queued --> shared AddToQueue
        |                                  reducer + DynamicQueue + notify
        |
        +-- provenance-owned pending unmark --> shared RemoveFromQueue
        |
        +-- active/wait/error/terminal/ineligible --> no queue side effect

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

- A fixed 10-second settle interval delays intentional single-mark admission. It is retained because stability was explicitly requested and avoids queue churn; configurability is future work.
- Bulk and single mark outcomes can arrive near deadline expiry. Serializing accepted outcomes and settlement through the existing application boundary ensures one ordering; tests must exercise both orderings deterministically.
- Provenance is process-local, so restart favors safety over automatic unmark removal. Queue intent remains reconstructible from workspace/reducer evidence and is not silently revoked.

## Migration Plan

- Replace canonical mark-only Running scenarios with stable mark-reconciliation scenarios.
- Add the coordinator and focused tests without changing existing queue/scheduler contracts.
- Reuse current `DynamicQueue`, reducer, queue hook, and reanalysis paths.
- Remove obsolete tests that assert every Running mark has no queue effect, replacing them with mode- and status-specific safety assertions.

## Open Questions

None. The proposal deliberately fixes the interval at 10 seconds and keeps active cancellation on `K`.
