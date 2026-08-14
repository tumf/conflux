## Context

A persistent scheduler parks in an event-driven idle wait while the TUI and Web project Ready plus `persistent_scheduler_idle: true`. F5 Start commits reducer queue intent and emits `RunDispatched`, but because the scheduler already exists the outcome carries `scheduler_started: false`. Current projection code interprets that as "nothing started" and waits for `WorkspacePreparationStarted` or another admitted-work event before restoring Running.

That distinction is accurate for actual agent execution but poor command feedback. Dependency analysis may take seconds, and the operator sees Ready after a command that was already accepted. The system already exposes actual activity separately through typed execution facts, so operator-visible mode does not need to carry the entire burden of proving lifecycle work.

## Goals

- Show Running immediately after an accepted persistent-idle Start.
- Preserve one authoritative transition across Core, TUI, Web, API, and lifecycle output.
- Keep rejected and no-op Start truthful.
- Preserve the same scheduler and existing queue/retry routing.
- Return to Ready if the wake produces no admitted work.
- Keep actual active-work and phase observation independent from application mode.

## Non-Goals

- Reducing dependency-analysis duration.
- Starting a replacement scheduler.
- Adding a new durable mode or workflow input.
- Making keypress receipt equivalent to Start acceptance.
- Redefining `has_active_work` from mode or queue status.

## Decision: Accepted Start Opens the Operator-Visible Run Episode

`OperatorCommandApplied::RunDispatched` already carries the committed target set and whether a new scheduler was spawned. The projection SHALL additionally use the existing persistent-idle episode fact at dispatch time:

| Current state | Accepted targets | Scheduler effect | Projection |
|---|---:|---|---|
| Pre-run Select | one or more | Started | Running, existing behavior |
| Persistent-idle Ready | one or more | Notified | Running immediately |
| Running | one or more retry targets | Notified | Running, unchanged |
| Any mode | none/refused/no-op | None | unchanged |

The transition happens after preparation and reducer commit, not in the key handler. Therefore Running still means the shared application transaction accepted executable intent, not merely that F5 was pressed.

The event remains the one source for TUI and Web. Reducer queue status travels in the same authoritative dispatch snapshot, so admitted rows become queued without a frontend-only write.

## Idle Episode Semantics

The accepted persistent-idle Start closes the presentation episode and clears `persistent_scheduler_idle` in Core/TUI/Web. This is intentionally earlier than workspace execution, but it is not evidence of an active phase.

The scheduler's edge latch must be able to publish Ready again if analysis admits no work. Rearming at the raw wake would be wrong because stop notifications, duplicate notifications, and generic queue wakeups may admit nothing. Rearming occurs only when the coherent scheduler pass observes at least one reducer queue addition or consumes an accepted explicit-retry edge.

After that rearm:

1. the accepted command projects Running;
2. the scheduler reconciles and analyzes the committed intent;
3. actual workspace/base-lane work emits its normal typed start events, or the scheduler reaches the persistent-idle predicate again;
4. a no-work park emits a fresh `PersistentSchedulerIdle` and projects Ready.

This preserves event-driven waiting and avoids a permanent Running state after an analyzer returns no usable order or all candidates become blocked.

## Truthful Activity Observation

Application mode answers what the operator command episode is doing. Execution facts answer whether work is actively consuming a lifecycle boundary.

The change SHALL preserve these distinctions:

- `app_mode: running` and external lifecycle `working` acknowledge accepted operator intent;
- `scheduler_running` continues to report scheduler task liveness;
- `has_active_work` remains false until a typed process activity such as `AnalysisStarted` or a reducer/base-lane lifecycle activity opens;
- no current phase is invented from Start acceptance, marks, queue intent, or mode;
- `PersistentSchedulerIdle`, terminal Error, and Stopped close the appropriate presentation episode through existing typed events.

## Stop and Cancellation Races

Clearing `persistent_scheduler_idle` at accepted Start means a later graceful stop originates from Running rather than idle Ready. This matches the new user-visible episode: Start was accepted and queued work belongs to the run. Cancel-stop therefore restores Running unless a subsequent idle event has already returned the process to Ready.

A stop that wins before Start acceptance remains governed by the existing application gate and mode validation. No new race is resolved in the key handler.

## API and Lifecycle Convergence

The accepted command outcome is state-owning and already advances the remote projection revision. The resulting revision SHALL contain:

- `app_mode: running`;
- `persistent_scheduler_idle: false`;
- admitted targets with reducer-derived queue intent;
- unchanged execution facts until their own typed events arrive.

External lifecycle projection SHALL emit `working` from the same mode transition. Unchanged frames remain deduplicated. If no work is admitted after analysis, the later persistent-idle event emits `idle` again.

## Verification Strategy

- Cross-adapter tests drive the same accepted outcome through Core, TUI, and Web and compare mode, idle fact, row queue status, scheduler calls, and revision.
- Refusal tests prove raw F5, missing marks, ineligible targets, and stale scheduler liveness do not project Running.
- Scheduler tests use paused/event-driven ordering to prove queue reconciliation rearms one idle edge and generic wakes do not.
- Retry tests cover terminal Error routes through the same accepted Start transaction.
- Execution-status tests assert the interval after Start acceptance and before `AnalysisStarted` has Running mode but no active work, then assert analysis opens process activity.
- No test uses a short wall-clock threshold as correctness evidence.

## Risks and Mitigations

- **Running could overstate agent execution:** execution-status keeps a separate typed `has_active_work` contract, and the UI rows remain queued until work starts.
- **No-work analysis could leave Running stuck:** coherent queue/retry reconciliation rearms the idle edge so the next park publishes Ready.
- **Generic wake could flicker Ready/Running:** only an accepted command outcome projects Running, and only observed queue/retry additions rearm idle.
- **Frontends could diverge:** all projections consume the same authoritative outcome and are tested together.
- **Stop cancellation could restore the old Ready state:** accepted Start clears the idle fact, so cancel-stop restores Running unless a later idle event reopened Ready.
