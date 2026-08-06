## Context

`SchedulerLifetime::Persistent` keeps the parallel scheduler task alive after coherent work detection proves there is nothing executable. `should_enter_persistent_idle_wait` already distinguishes this state from transient reducer unreadability and from finite termination, but the transition into the wait is visible only in logs.

TUI and Web each retain a process-local execution mode. Reducer-owned change status remains authoritative for rows, while the frontend mode answers whether orchestration is actively executing. The missing transition is therefore a typed process-level presentation event, not a new durable workflow state.

## Goals

- Project persistent idle as Ready across TUI, Web, `/api/v2`, and external lifecycle output.
- Preserve the scheduler's event-driven lifetime and all reducer/workspace evidence.
- Emit one transition per real idle episode.
- Restore Running only from typed evidence that admitted work began.

## Non-Goals

- Reusing terminal completion or stop events.
- Persisting idle state or using it for workflow routing.
- Changing queue, blocker, retry, worktree, or execution-mark semantics.
- Introducing idle polling.

## Decision: Add a Distinct Persistent-Idle Event

Introduce a process-level event dedicated to the scheduler parking while still alive. It is classified through the exhaustive execution-event ownership table and delivered by the existing dispatch owner to every frontend.

The reducer applies no workflow mutation for this event. It remains a state-owning dispatch because frontend `app_mode` and the API operator snapshot change at that dispatch. This matches existing process-level mode events while preserving one ordered event and one coherent projection revision.

`AllCompleted` is rejected because it means the run reached a terminal completion boundary and can add success presentation or cleanup. `Stopped` is rejected because it changes command admission and cancellation semantics. The new event means only: the persistent scheduler is alive but currently has no executable work.

## Idle Eligibility and Edge Trigger

The producer reuses `should_enter_persistent_idle_wait` and its coherent reducer snapshot. It does not introduce a second drain predicate.

An idle-episode latch follows these rules:

1. Emit immediately before the first park in an idle episode.
2. Keep the latch set across repeated loop evaluations and wake notifications that do not begin admitted work.
3. Rearm only after a typed admitted-work transition begins ordinary workspace preparation or a scheduler-owned base-lane operation.
4. Cancellation and terminal stop retain their existing events and do not synthesize another idle transition.

This prevents an empty notification from creating `Ready`, `Running`, `Ready` churn.

## Frontend Transition Matrix

| Current frontend mode | Persistent idle result |
|---|---|
| `Running` | Ready / `select` |
| `Select` | unchanged |
| `Stopping` | unchanged |
| `Stopped` | unchanged |
| `Error` | unchanged |

The handler changes only execution mode and run-level active presentation. It does not call successful-completion helpers and does not rewrite rows, queue intent, marks, elapsed change evidence, blocker details, or diagnostics.

Fully drained and blocked/waiting-only idle use the same Ready mode. Blocked/stalled/resolve-pending/reject-pending rows remain visible, but they do not make an idle scheduler claim active execution.

## Resume Projection

Ready ends only when an existing typed event proves work has crossed an admission boundary. Ordinary work uses `WorkspacePreparationStarted`. Resolve and rejection/base-lane work use their typed start/status transitions. TUI and Web apply the same guarded Ready-to-Running rule.

`AnalysisStarted`, queue notifications, and catalog refresh are not execution evidence and leave Ready unchanged.

## API and Lifecycle Projection

The Web sink applies the idle event before building the `/api/v2` candidate snapshot. The event envelope and snapshot therefore carry `app_mode: select` at one state revision. Duplicate/no-op delivery produces no additional revision.

The direct execution-event lifecycle mapping and TUI typed lifecycle snapshot both project Ready as `idle`. A blocked/stalled row retained under Ready does not change that process-level fact. Admitted-work events project `working` again.

## Verification Strategy

- Scheduler unit tests hold the coherent drain/blocked inputs constant and prove one idle event per episode.
- A no-op wake test proves notification without admitted work does not rearm the event.
- TUI tests prove only Running becomes Select and no completion message or row mutation occurs.
- Web event-ownership tests prove one coherent Ready revision and duplicate idempotency.
- Resume tests cover ordinary workspace preparation and scheduler-owned base-lane work.
- Lifecycle tests prove idle/working transitions without rendered-screen parsing.

## Risks and Mitigations

- **Ready is mistaken for scheduler termination:** API/event documentation states the scheduler remains alive; tests assert the next explicit wake is consumed by the same scheduler.
- **A no-op wake causes mode flicker:** the idle latch rearms only on admitted work.
- **Terminal modes are overwritten:** handlers use an explicit transition matrix and tests cover Error, Stopping, and Stopped.
- **Blocked evidence disappears:** the event owns no row or reducer mutation; before/after snapshots pin every retained field.
- **Resume starts invisibly:** existing admitted-work events become the shared Running trigger instead of adding a second scheduler-start signal.
