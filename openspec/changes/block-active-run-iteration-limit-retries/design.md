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
- Make run closure and retry admission race-free.
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

A typed `ApplyIterationLimit { change_id, attempts, max }` record whose owning boundary is still open for operator admission. The record is not a durable blocker and is not workspace evidence.

### Later boundary

A scheduler boundary admitted only after the prior boundary has closed. It creates or installs fresh active-run state and re-evaluates the preserved workspace. It is not a wake-up of the prior scheduler.

## Decision 1: Keep typed evidence as the only retry-gate input

The retry gate is derived from the reducer's typed `ApplyIterationLimit` record plus the owning boundary's active lifetime. The implementation must not inspect `error_detail`, logs, status-bar text, or a formatted max-iterations error.

Expose one shared query equivalent to:

```text
active_apply_iteration_limit(change_id) -> Option<{ attempts, max }>
```

Operator commands, TUI projection, and v2 projection consume this query. They do not each recreate the lifetime rule.

## Decision 2: Guard before mutation

For an individual target, admission order is:

1. Acquire the shared operator/run-boundary admission guard.
2. Read active typed limit evidence for the target.
3. If present, return a typed refusal.
4. Only otherwise classify the ordinary retry route and mutate reducer, failed classification, marks, queue, hooks, explicit-retry edges, or scheduler state.

This order also applies when queue addition would treat a terminal-error row as `RetryError`. A caller cannot bypass the guard by sending `set_queue_intent=true` instead of `retry_change`.

The refusal should remain machine-readable end to end. The internal error/exclusion type and the v2 `ActionBlockedReason` may use different Rust enums, but both represent the stable semantic token `apply_iteration_limit_active` rather than prose.

## Decision 3: Bulk retry is filtering, not all-or-nothing

Bulk retry takes one coherent admission snapshot under the same guard. For each candidate it records one of:

- accepted ordinary retry route;
- excluded because an active Apply limit owns the target;
- excluded by existing unsupported or non-resumable evidence.

Accepted targets are mutated and dispatched once. Limited targets are untouched. If no target is accepted, the result is `NoRetryableTarget`/no-op and the scheduler receives neither notify nor start.

Bulk result detail should retain stable exclusion reasons where the existing outcome surface supports them. It must never make a limited row look accepted merely because another row dispatched.

## Decision 4: Close the run through a serialized barrier

A run that records the limit follows this order:

1. The budget owner records typed `ApplyIterationLimit` with the exact count.
2. Frontends may observe the failed row and active retry block.
3. The run boundary reads the record and invokes its existing sole `on_finish` owner with `iteration_limit` and the exact Apply count.
4. After the hook attempt returns, the boundary enters a closing barrier shared with operator admission.
5. Under that barrier, it publishes terminal reducer/frontend state, retires the active limit gate, and makes the old scheduler unavailable for notification as one ordered lifecycle transition.
6. It performs no later reducer or frontend mutation after releasing that barrier.
7. A subsequent command may then create a later boundary with fresh active-run state.

Hook failure does not preserve the gate forever. The ordering requirement is that the hook attempt observed the record, not that an external hook command succeeded.

The implementation may use a run generation, an active-boundary lease, or an equivalent process-local primitive. It must not rely on a timing assumption between clearing a vector and `JoinHandle::is_finished`. A deterministic concurrency regression must pause the closing transition and prove there is no interval in which retry mutates old state while dispatch still targets the old scheduler.

## Decision 5: Later runs replace, never repair, the budget

No retry command mutates `ApplyBudget`. Once the old boundary is closed, a later admitted run follows the existing initialization path that replaces active-run `OrchestratorState` and creates a new executor budget. Workspace and Git evidence select the next workflow operation.

The same-process case and process-restart case have the same authority model:

- old ephemeral counter and gate are absent from the new boundary;
- preserved worktree files and Git state remain;
- no API snapshot, log, or local-state artifact is read back as control input.

## Decision 6: Project typed data, not inferred permission

### API

Each change resource gains nullable active iteration-limit evidence containing `attempts` and `max`. While present, `actions.retry_change` is:

```json
{
  "allowed": false,
  "blocked_reason": "apply_iteration_limit_active"
}
```

The evidence and action eligibility must come from one coherent reducer/run-lifetime observation and appear at the same `state_revision`. Generated OpenAPI reflects both the evidence object and enum value; no tracked schema file is added.

### WebUI

`changeActions` renders Retry only when `change.actions.retry_change.allowed` is true. `display_status === "error"` is presentation, not authorization. Browser tests cover a blocked error row and a later allowed snapshot.

### TUI

The TUI synchronizes a process-local per-change limit eligibility cache from the same shared state used by the command service. Row Space handling, bulk selection, F5 retry, and footer/row guidance consult that cache. The diagnostic remains visible, but retry-promising guidance is replaced by a stable explanation while blocked.

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

### Bulk routing

One test combines a limited terminal error, an ordinary terminal error, and a resumable acceptance hold. It proves only the latter two mutate and dispatch once. An all-limited test proves no reducer, queue, edge, mark, hook, notify, or spawn effect.

### Boundary ordering

A deterministic test gate pauses the run-closing barrier after `on_finish` reads the typed record. A concurrent retry must remain blocked or return the typed refusal until closure commits. After release, retry must start a new scheduler generation, never notify the old generation. The new state begins with a fresh budget while preserving workspace-derived routing evidence.

### API and OpenAPI

Projection tests assert typed `{ attempts, max }`, the blocked reason, coherent retirement, and ordinary-error behavior. The generated OpenAPI contract test serializes the new object and enum token.

### TUI and browser

Cross-adapter/TUI tests prove Space, bulk, and F5 paths have no command or mark effect and render no retry promise while active. Vitest proves the console ignores `display_status=error` when server eligibility blocks Retry, then restores the action when the next snapshot allows it.

All new unit tests use in-memory state and deterministic synchronization. No test requires network access, credentials, wall-clock sleeps, or durable local state.
