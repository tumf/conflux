# Design: Pure next-run execution marks

## Decision

Execution marks become one process-local boolean intent per change: include this change when a later run command evaluates targets. The mark write path does not decide whether the change is runnable and does not mutate any current-run mechanism.

## Shared Contract

The existing `ExecutionMarkStore` remains the authority. TUI rows and `/api/v2` snapshots remain projections of that store.

A shared classifier distinguishes only:

- visible non-terminal target: mark mutation allowed;
- archived, merged, pushed, or rejected target: mark mutation unavailable because the row is not a run candidate.

Execution mode, active status, error/retry status, wait state, Apply-limit state, queue intent, and parallel eligibility are deliberately absent from mark admission. Rejected marker rows retain their existing read-only contract and may become markable only after the marker is removed and discovery restores a non-terminal row.

## Command Separation

Mark commands mutate only `ExecutionMarkStore`. They do not call `QueuePort`, queue hooks, cancellation ports, retry/resolve services, scheduler ports, or reducer queue commands.

Existing controls keep their independent meanings:

- Space: mark/unmark future run intent;
- `x`: apply one mark state to all visible non-terminal rows;
- `K`: terminate one active change through the existing guarded flow;
- configured start key: admit and dispatch current marked targets;
- explicit queue API: mutate DynamicQueue when a client intentionally invokes a queue command;
- retry/resolve controls: create their existing typed recovery intent.

## Run Admission

Start/retry reads one coherent mark snapshot, then evaluates current reducer/worktree facts. No mark-time eligibility result is reused as workflow authority.

Admission is fail-before-effect:

1. capture marked IDs;
2. apply the existing all-or-nothing worktree eligibility fence to the complete marked set;
3. classify remaining IDs by the requested route, excluding non-startable statuses with target-specific diagnostics;
4. reject when no startable target remains;
5. prepare scheduler capability;
6. commit any required run intent and publish the accepted outcome;
7. activate the prepared scheduler.

Configured Start does not select ordinary-start versus retry solely from process mode. After the worktree fence, it classifies the coherent marked snapshot by target route:

- retry-eligible recovery rows use the existing typed retry routes in Ready/Select, Stopped, and process-wide Error;
- when at least one retry route exists, this invocation dispatches only those routes with explicit-retry semantics;
- marked ordinary-start rows in that mixed request are reported as deferred and retain their marks for a later ordinary Start;
- when no retry route exists, startable `not queued` rows use the existing ordinary Start route;
- Running and Stopping continue to refuse configured Start.

This priority prevents a run-wide `explicit_retry` flag from granting retry-only acceptance budget to fresh ordinary work. A worktree-ineligible marked row rejects the complete request; other currently non-startable statuses do not block runnable work but are reported with target-specific detail. Any rejection is effect-free.

This preserves the existing atomic command boundary while removing mark/queue aliasing. It also closes the Ready/Select gap after a change-scoped `ProcessingError`: re-mark plus configured Start/F5 reaches the typed retry path even though Core correctly never entered process-wide Error. Unmarking after admission affects only a later run and cannot cancel work already admitted.

## Archive Boundary

Existing typed reconciliation edges continue to revoke stale marks for failure, rejection, rejected or parallel-ineligible refresh, dequeue, target-scoped stop, and first `on_merged` hook recovery. Those system revocations do not prohibit an operator from marking a later non-terminal steady state again.

`ChangeArchived` is added as the terminal edge where mark intent stops having meaning. The authoritative dispatch reconciler clears that target's mark after reducer application and before frontend projection, in the same revision. Duplicate or stale archive events do not create another revision or clear unrelated marks. Merged and pushed events preserve the already-cleared state. Restart also begins with an empty store as before.

## Rendering

The Changes list keeps its existing prefix width. For `archived`, `merged`, and `pushed` rows, rendering substitutes spaces with the same display width as `[x]`/`[ ]` rather than removing the prefix. Preview-width calculations use that same fixed width, so every later column stays aligned.

Post-archive rows expose no Space or bulk mark affordance. Space is consumed as a silent no-op because the row remains visible for session history but has no mark semantics.

## Verification Strategy

Focused in-memory service tests prove all non-terminal lifecycle classes accept mark-only mutation and record no queue/runtime effects while terminal rows remain unchanged no-ops. Cross-adapter tests prove TUI and API parity. Run-control tests prove eligibility is final-admission-owned and failed admission is effect-free. Event/revision tests prove archive and mark reconciliation are coherent. Buffer tests assert both absence of checkbox glyphs and exact column offsets.
