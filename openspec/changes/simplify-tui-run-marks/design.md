# Design: Pure next-run execution marks

## Decision

Execution marks become one process-local boolean intent per change: include this change when a later run command evaluates targets. The mark write path does not decide whether the change is runnable and does not mutate any current-run mechanism.

## Shared Contract

The existing `ExecutionMarkStore` remains the authority. TUI rows and `/api/v2` snapshots remain projections of that store.

A shared classifier distinguishes only:

- visible pre-archive target: mark mutation allowed;
- archived, merged, or pushed target: mark mutation unavailable because archive ended its run candidacy.

Execution mode, active status, error/retry status, wait state, Apply-limit state, queue intent, and parallel eligibility are deliberately absent from mark admission.

Rejected marker rows remain pre-archive rows. They may carry future-run intent, but the mark itself does not bypass rejection recovery or certify run eligibility. Final run admission still rejects or routes them according to current workflow facts.

## Command Separation

Mark commands mutate only `ExecutionMarkStore`. They do not call `QueuePort`, queue hooks, cancellation ports, retry/resolve services, scheduler ports, or reducer queue commands.

Existing controls keep their independent meanings:

- Space: mark/unmark future run intent;
- `x`: apply one mark state to all visible pre-archive rows;
- `K`: terminate one active change through the existing guarded flow;
- configured start key: admit and dispatch current marked targets;
- explicit queue API: mutate DynamicQueue when a client intentionally invokes a queue command;
- retry/resolve controls: create their existing typed recovery intent.

## Run Admission

Start/retry reads one coherent mark snapshot, then evaluates current reducer/worktree facts. No mark-time eligibility result is reused as workflow authority.

Admission is fail-before-effect:

1. capture marked IDs;
2. classify current target eligibility and route;
3. reject an empty or invalid requested set with target-specific reasons;
4. prepare scheduler capability;
5. commit any required run intent and publish the accepted outcome;
6. activate the prepared scheduler.

This preserves the existing atomic command boundary while removing mark/queue aliasing. Unmarking after step 5 affects only a later run and cannot cancel work already admitted.

## Archive Boundary

`ChangeArchived` is the edge where mark intent stops having meaning. The authoritative dispatch reconciler clears that target's mark after reducer application and before frontend projection, in the same revision. Duplicate or stale archive events do not create another revision or clear unrelated marks.

Merged and pushed events preserve the already-cleared state. Restart also begins with an empty store as before.

## Rendering

The Changes list keeps its existing prefix width. For `archived`, `merged`, and `pushed` rows, rendering substitutes spaces with the same display width as `[x]`/`[ ]` rather than removing the prefix. Preview-width calculations use that same fixed width, so every later column stays aligned.

Post-archive rows expose no Space or bulk mark affordance. Space is consumed as a silent no-op because the row remains visible for session history but has no mark semantics.

## Verification Strategy

Focused in-memory service tests prove all lifecycle classes accept pre-archive mark-only mutation and record no queue/runtime effects. Cross-adapter tests prove TUI and API parity. Run-control tests prove eligibility is final-admission-owned and failed admission is effect-free. Event/revision tests prove archive and mark reconciliation are coherent. Buffer tests assert both absence of checkbox glyphs and exact column offsets.
