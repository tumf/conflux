## Context

`SchedulerLifetime::Persistent` keeps the parallel scheduler task alive after coherent work detection proves there is nothing executable. `should_enter_persistent_idle_wait` already distinguishes this state from transient reducer unreadability and from finite termination, but the transition into the wait is visible only in logs.

TUI and Web each retain a process-local execution mode. Reducer-owned change status remains authoritative for rows, while the frontend mode answers whether orchestration is actively executing. The missing transition is therefore a typed process-level presentation event, not a new durable workflow state.

## Goals

- Project persistent idle as Ready across TUI, Web, `/api/v2`, and external lifecycle output.
- Expose one process-local idle-episode fact so TUI and Web can distinguish live-idle Ready from pre-run Select.
- Preserve the scheduler's event-driven lifetime and all reducer/workspace evidence.
- Emit one transition per real idle episode.
- Keep a live persistent scheduler command-addressable while it is presented as Ready, and restore Running only when that scheduler begins admitted work.

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

The handler changes execution mode, run-level active presentation, and a process-local `persistent_scheduler_idle` idle-episode fact. It does not call successful-completion helpers and does not rewrite rows, queue intent, marks, elapsed change evidence, blocker details, or diagnostics.

The fact is frontend/API observation state, not scheduler liveness or workflow authority. It defaults false and is discarded on process restart. It becomes true only when the typed idle event performs the guarded Running-to-Ready transition; a late event against Select, Stopping, Error, or Stopped leaves both mode and fact unchanged. Once true, it remains true while Start merely notifies the scheduler and while an idle-origin graceful stop is pending, and becomes false when typed admitted work starts or Error/Stopped terminates the episode. Cancel-stop therefore restores Ready when the fact remains true.

Fully drained and blocked/waiting-only idle use the same Ready mode. Blocked/stalled/resolve-pending/reject-pending rows remain visible, but they do not make an idle scheduler claim active execution.

## Live-Scheduler Command Boundary

TUI and Web distinguish persistent-idle Ready from pre-run Select with `persistent_scheduler_idle`; no new durable mode is added. That observation makes the existing controls discoverable, but shared run control SHALL independently consult scheduler `is_running()` when executing them:

- mark and bulk-mark remain the existing Select-mode mark-only mutations;
- Start resolves the authoritative marked targets, applies existing reducer queue intent, and notifies the live scheduler instead of spawning another task;
- when Start returns `SchedulerEffect::Notified`, TUI and Web remain Ready and retain `persistent_scheduler_idle` until typed work-start evidence arrives; the existing synchronous `begin_run` projection remains unchanged only for `SchedulerEffect::Started`;
- TUI's first Esc and Web's graceful-stop action remain available in Ready when `persistent_scheduler_idle` is true; force stop remains available through the existing second-Esc/explicit destructive action;
- graceful stop and force stop remain accepted against the live scheduler despite Ready presentation, and graceful stop notifies the idle waiter after setting the stop request so it can reach the existing stop boundary;
- cancel-stop remains accepted only after graceful stop has projected Stopping and restores Ready instead of Running when the idle-episode fact remains true.

The idle-episode fact is a process-local presentation discriminator. Scheduler `is_running()` remains the command-admission authority, so a stale client fact cannot authorize work against an exited scheduler.

## Resume Projection

Ready ends when an existing typed event proves work has crossed an execution boundary. Ordinary work uses `WorkspacePreparationStarted`. Resolve and rejection/base-lane work use their typed start/status transitions. TUI and Web apply the same guarded rule: clear `persistent_scheduler_idle`, change Select to Running, and preserve Stopping if a graceful-stop request arrived first. Cancel-stop restores Ready only while the fact remains true; after work-start cleared it, cancel-stop restores Running.

`AnalysisStarted`, Start notification, queue notification, and catalog refresh are not execution evidence and leave Ready unchanged.

## API and Lifecycle Projection

The Web sink applies the idle event before building the `/api/v2` candidate snapshot. Under the canonical `remote-control-api` serialized optimistic revision contract, the event envelope and snapshot therefore carry `app_mode: select` and `persistent_scheduler_idle: true` at one state revision. The field is part of the generated OpenAPI schema. Duplicate/no-op delivery produces no additional revision.

The authoritative dispatch lifecycle projection and TUI typed lifecycle snapshot both project an accepted Ready transition as `idle`. The event variant alone is insufficient: a late persistent-idle event whose guarded transition is rejected publishes no new idle state. A blocked/stalled row retained under Ready does not change that process-level fact. Admitted-work events project `working` again and clear the idle-episode fact coherently.

## Verification Strategy

- Scheduler unit tests hold the coherent drain/blocked inputs constant and prove one idle event per episode.
- A no-op wake test proves notification without admitted work does not rearm the event.
- TUI tests prove only Running becomes Select, `persistent_scheduler_idle` controls the idle Ready Esc path, and no completion message or row mutation occurs.
- Web event-ownership tests prove one coherent Ready revision, generated OpenAPI field ownership, duplicate idempotency, and Start/stop/force-stop control visibility distinct from pre-run Select.
- Resume tests cover ordinary workspace preparation and scheduler-owned base-lane work, including coherent idle-fact clearing.
- Command tests prove idle Ready marks stay mark-only, Start applies queue intent and notifies the same scheduler without premature Running, graceful/force stop remain effective, and cancel-stop restores Ready for an idle-origin stop.
- Lifecycle tests prove idle/working transitions without rendered-screen parsing.

## Risks and Mitigations

- **Ready is mistaken for scheduler termination:** API/event documentation states the scheduler remains alive; tests assert the next explicit wake is consumed by the same scheduler.
- **A no-op wake causes mode flicker:** the idle latch rearms only on admitted work.
- **Terminal modes are overwritten:** handlers use an explicit transition matrix and tests cover Error, Stopping, and Stopped.
- **Blocked evidence disappears:** the event owns no row or reducer mutation; before/after snapshots pin every retained field.
- **Ready makes the live scheduler look terminated to run control:** command admission pairs Ready/`Select` presentation with `scheduler.is_running()` and tests Start plus both stop paths.
- **Start notification falsely claims execution:** `SchedulerEffect::Notified` leaves Ready unchanged; existing admitted-work events become the shared Running trigger.
