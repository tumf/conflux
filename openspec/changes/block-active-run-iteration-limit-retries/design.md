# Design: Active-run Apply-limit retry admission

## Context

One active worktree scheduler boundary owns three related facts:

1. The cumulative per-change Apply dispatch count.
2. The `ApplyBudget` that refuses dispatch after `max_iterations`.
3. Typed `ApplyIterationLimit` evidence used by the boundary's `on_finish` hook.

The retry service currently owns none of that context. It sees an `error` row, applies `ReducerCommand::RetryError`, publishes an explicit-retry edge, marks the row, and asks the scheduler to run. When the original boundary is still alive, those mutations cannot replenish its exhausted budget. The scheduler immediately observes the same ceiling.

The record cannot simply be permanent. `OrchestratorState` is active-run state and is replaced for a later scheduler boundary. Conflux's constitution also requires restart routing to come from workspace and Git evidence, not hidden durable state.

## Goals

- Reject every path that would retry an iteration-limited change into its owning active boundary.
- Preserve the typed record until the sole finish-hook attempt consumes it.
- Use the existing scheduler-task lifetime to keep run closure and retry admission race-free.
- Keep bulk retry useful for unrelated retryable changes.
- Publish one typed eligibility contract to API, WebUI, and TUI.
- Allow a later scheduler boundary to start from workspace evidence with a fresh budget.

## Non-Goals

- Change the configured limit or reset the current boundary's counter.
- Persist limit evidence or introduce a run journal.
- Automatically start a replacement run.
- Parse diagnostic strings into control state.
- Redesign ordinary terminal-error, acceptance-stall, or external-hold retry routing.

## Terminology

### Owning boundary

The scheduler boundary that created the current `OrchestratorState`, reserved the Apply dispatches, recorded `ApplyIterationLimit`, and owns the matching `on_finish` call.

### Active iteration-limit gate

A typed `ApplyIterationLimit { change_id, attempts, max }` record whose owning scheduler task still reports live through `RunSchedulerPort::is_running()`. The record is not a durable blocker and is not workspace evidence.

### Later boundary

A scheduler boundary admitted only after the prior boundary has closed. It creates or installs fresh active-run state and re-evaluates the preserved workspace. It is not a wake-up of the prior scheduler.

## Decision 1: Use typed evidence and task liveness as the only gate inputs

The retry gate is derived from the reducer's typed `ApplyIterationLimit` record plus the owning scheduler task's `RunSchedulerPort::is_running()` result. The implementation must not inspect `error_detail`, logs, status-bar text, or a formatted max-iterations error.

Expose one shared query equivalent to:

```text
active_apply_iteration_limit(change_id) -> Option<{ attempts, max }>
```

Operator commands, TUI projection, and command-capable v2 projection consume this query. They do not each recreate the lifetime rule. Headless `cflx run` has no bound command executor, so its read-only projection uses an explicit degraded boundary-liveness rule and does not treat retained record presence alone as an active gate.

## Decision 2: Guard before mutation

For an individual target, admission order is:

1. Evaluate the shared active-limit query from typed evidence and scheduler-task liveness.
2. If active evidence is present, return a typed refusal.
3. Only otherwise classify the ordinary retry route and mutate reducer, failed classification, marks, queue, hooks, explicit-retry edges, or scheduler state.

This order also applies when queue addition would treat a terminal-error row as `RetryError`. A caller cannot bypass the guard by sending `set_queue_intent=true` instead of `retry_change`.

The refusal should remain machine-readable end to end. The internal error/exclusion type and the v2 `ActionBlockedReason` may use different Rust enums, but both represent the stable semantic token `apply_iteration_limit_active` rather than prose.

## Decision 3: Bulk classification excludes active limits before mutation

Bulk execution-mark planning and bulk retry take one coherent eligibility snapshot. Bulk-mark planning adds `apply_iteration_limit_active` to `MarkExclusion` and excludes a limited terminal-error row before any eligible mark or Running queue intent is applied. This prevents the terminal-error `add_to_queue` guard from aborting a partially applied bulk operation.

For bulk retry, each candidate is one of:

- accepted ordinary retry route;
- excluded because an active Apply limit owns the target;
- excluded by existing unsupported or non-resumable evidence.

Accepted targets are mutated and dispatched once. Limited targets are untouched. If no target is accepted, the result is `NoRetryableTarget`/no-op and the scheduler receives neither notify nor start. The existing `RetryPlan` and `RunControlOutcome` need no per-target exclusion DTO: a limited target is never claimed accepted, and its stable blocked reason remains readable in the authoritative snapshot at the command's result revision.

## Decision 4: Use scheduler-task liveness as the command-capable boundary primitive

A TUI/remote command-capable run that records the limit follows this order:

1. The budget owner records typed `ApplyIterationLimit` with the exact count.
2. Frontends may observe the failed row and active retry block while `RunSchedulerPort::is_running()` is true.
3. The run task reads the record and invokes its existing sole `on_finish` owner with `iteration_limit` and the exact Apply count.
4. The task publishes its existing terminal reducer/frontend events and then returns.
5. `JoinHandle::is_finished()` makes `RunSchedulerPort::is_running()` false, retiring the gate without clearing the record.
6. The TUI loop observes that liveness transition, refreshes its eligibility cache, and asks `WebState` to publish the changed action snapshot once.
7. A subsequent command may create a later boundary with fresh active-run state.

The task handle remains live throughout `on_finish` and terminal publication, including when the hook reports an error. Therefore a limited retry is refused for the entire interval in which it could notify the old scheduler. Once the task exits, that scheduler cannot resurrect: dispatch can only observe a live newly started task or start a new boundary. No close operation, lock, run generation, record clearing, or closing barrier is needed.

A closing barrier is rejected because terminal publication uses bounded event channels. Holding an admission lock while publishing could deadlock against an event loop waiting for that same lock in a command handler. Tests instead drive scheduler liveness deterministically with `running=true` and `running=false`, verify that `on_finish` observes the record, and prove later admission never notifies the exited task. Headless `cflx run` has no admission surface, so its only obligation is the existing regression that the record survives until its finish hook returns.

## Decision 5: Later runs replace, never repair, the budget

No retry command mutates `ApplyBudget`. Once the old boundary is closed, a later admitted run follows the existing initialization path that replaces active-run `OrchestratorState` and creates a new executor budget. Workspace and Git evidence select the next workflow operation.

The same-process case and process-restart case have the same authority model:

- old ephemeral counter and gate are absent from the new boundary;
- preserved worktree files and Git state remain;
- no API snapshot, log, or local-state artifact is read back as control input.

## Decision 6: Project typed eligibility, not inferred permission

### API

While the shared active-limit query returns evidence, `actions.retry_change` is:

```json
{
  "allowed": false,
  "blocked_reason": "apply_iteration_limit_active"
}
```

Typed record presence and scheduler-task liveness must come from one coherent command-capable boundary observation and publish action eligibility at one `state_revision`. `WebState` receives the same scheduler-liveness authority used by admission. Generated OpenAPI reflects the enum value; no unused per-change evidence DTO or tracked schema file is added. A headless `cflx run` API has no command executor, so it remains read-only instead of projecting a stale actionable block after task exit.

### WebUI

`changeActions` renders Retry only when `change.actions.retry_change.allowed` is true. `display_status === "error"` is presentation, not authorization. Browser tests cover a blocked error row and a later allowed snapshot.

### TUI

The TUI synchronizes a process-local per-change limit eligibility cache from the same shared query used by the command service. Row Space handling, bulk selection, F5 retry, and footer/row guidance consult that cache. The diagnostic remains visible, but retry-promising guidance is replaced by a stable explanation while blocked. This cache is required because row Space handling optimistically flips a mark before command dispatch; without the pre-dispatch guard the UI would briefly claim intent the service refuses.

TUI state is presentation and ephemeral. It cannot become the service guard; direct API/service calls remain protected without a TUI.

## Alternatives Rejected

### Reset the counter on RetryError

This makes `max_iterations` a per-click limit rather than an active-run safety ceiling and permits unbounded operator-driven loops. It also splits budget ownership across the executor and command service.

### Persist the limit as a durable blocker

This violates workspace-local routing and makes local state deletion or API history affect the next action. A later run would be unable to recover from a preserved worktree without an out-of-band clear operation.

### Reject from WebUI and TUI only

Remote API clients and the direct queue alias would still mutate the exhausted run. Frontends are consumers of eligibility, not the authority.

### Clear the record immediately after recording or before `on_finish`

The finish hook would lose its typed status or exact Apply count and could regress to log parsing.

### Clear the record at an unsynchronized task tail

A retry could observe no record while `RunSchedulerPort::is_running` still points at the closing scheduler, causing the same lost or no-progress dispatch through a narrower race.

### Reject every bulk retry when one target is limited

Unrelated recoverable changes do not share the exhausted target's budget. Rejecting them would turn one per-change safety gate into a global run blocker.

## Verification Design

### Service mutation snapshots

Unit tests arrange a reducer error, mark store, queue double, hook double, explicit-retry recorder, and scheduler recorder. They capture every value before retry and assert exact equality after an active-limit refusal. A separate test drives the terminal-error `add_to_queue` alias.

### Bulk classification and routing

A bulk-mark test combines a limited terminal error with unrelated eligible rows. It proves the limited row receives `MarkExclusion::ApplyIterationLimitActive`, remains untouched, and cannot abort atomic application to eligible rows. A bulk-retry test combines a limited terminal error, an ordinary terminal error, and a resumable acceptance hold. It proves only the latter two mutate and dispatch once and the result does not claim the limited row was accepted. An all-limited test proves no reducer, queue, edge, mark, hook, notify, or spawn effect.

### Boundary ordering

Deterministic service tests set `RecordingScheduler` liveness true and false. They prove active evidence blocks while true, task exit retires the gate while retaining the record, and later admission starts rather than notifies. Existing finish-hook ordering fixtures prove the TUI owner observes the exact record before task completion and the CLI owner observes it before the run returns. The new state begins with a fresh budget while preserving workspace-derived routing evidence.

### API and OpenAPI

Projection tests assert the blocked reason while evidence and scheduler liveness are active, retirement after task exit, ordinary-error behavior, and the headless read-only rule. The generated OpenAPI contract test serializes the new enum token without adding an evidence object.

### TUI and browser

Cross-adapter/TUI tests prove Space, bulk, and F5 paths have no command or mark effect and render no retry promise while active. Vitest proves the console ignores `display_status=error` when server eligibility blocks Retry, then restores the action when the next snapshot allows it.

All new unit tests use in-memory state and deterministic synchronization. No test requires network access, credentials, wall-clock sleeps, or durable local state.
