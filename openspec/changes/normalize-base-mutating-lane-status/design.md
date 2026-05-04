# Design: Base-mutating lane status normalization

## Premise

`Resolving` and `Rejecting` both can touch the base branch/workspace. Even when the visible operation differs, they share one operational constraint: only one can safely run at a time. The reducer should therefore model a single base-mutating lane while preserving operation-specific pending statuses.

## State model

The reducer should distinguish:

| Concept | Meaning | Display |
|---|---|---|
| `ActivityState::Resolving` | archive-merge / conflict resolve is actively using the base-mutating lane | `resolving` |
| `ActivityState::Rejecting` | rejection review is actively using the base-mutating lane | `rejecting` |
| `WaitState::ResolveWait` | merge/resolve work is waiting for the base-mutating lane | `resolve pending` |
| `WaitState::RejectWait` | rejection-review work is waiting for the base-mutating lane | `reject pending` |
| `WaitState::MergeWait` | merge work has a manual blocker | `merge wait` |

`ResolveWait` and `RejectWait` are both auto-resumable, but they are not interchangeable. The scheduler must know which operation to start after the lane clears.

## Lane ownership rule

At any point, among all non-terminal changes:

```text
count(activity in {Resolving, Rejecting}) <= 1
```

This rule should be enforced through reducer transitions and scheduler checks, with tests that fail if two active lane occupants can be produced.

## Dispatch decision table

### Archive-complete merge work

| Condition | Result |
|---|---|
| another non-terminal change is `Resolving` or `Rejecting` | `ResolveWait` / `resolve pending` |
| concrete manual merge blocker exists | `MergeWait` / `merge wait` |
| no lane/manual blocker | `Resolving` then `Merged` |

### Rejection-review handoff

| Condition | Result |
|---|---|
| another non-terminal change is `Resolving` or `Rejecting` | `RejectWait` / `reject pending` |
| no lane blocker | `Rejecting` |
| rejection review completes with `Confirm` | terminal `Rejected` |
| rejection review completes with `Resume` | returns to `Applying` |
| rejection review completes with `Block` | non-terminal stalled/blocked state per existing rejecting-review semantics |

## Promotion after lane clear

When the active lane operation completes or fails, the scheduler should evaluate pending lane queues deterministically. The chosen ordering must be stable and documented in tests. A simple acceptable policy is insertion order across a unified pending-lane queue with operation type attached. If implementation keeps separate `resolve_wait` and `reject_wait` queues, it must still define deterministic priority and avoid starvation.

Promotion must start exactly one pending base-mutating operation at a time.

## UI/Web synchronization

TUI and Web should not maintain independent lifecycle truth for base-lane statuses. Local event handlers may update non-authoritative metadata and logs, but visible status for these states should come from reducer output:

- `resolving`
- `rejecting`
- `resolve pending`
- `reject pending`
- `merge wait`
- `merged`
- `rejected`
- `error`

In parallel mode, `archived` is an archive milestone, not a stable visible lifecycle state after post-archive routing begins. Serial mode may continue to use terminal `archived`.

## Constitution compliance

The new pending state must not become out-of-worktree durable workflow control. It is reducer/scheduler intent derived from observed workspace/git/base-tree facts and runtime lane occupancy. Resume routing after process restart must still derive the next action from workspace-local facts, consistent with `openspec/CONSTITUTION.md`.

## Alternatives considered

### Reuse `resolve pending` for rejection-review waits

Rejected. This loses the operation type and risks starting merge retry when the next required operation is rejection review.

### Treat rejecting as manual merge wait blocker

Rejected. A running rejection review is auto-resumable once the lane clears. It should not require the user to press `M` or resolve a manual blocker.

### Allow resolving and rejecting concurrently if they touch different files

Rejected. Both can mutate base state and dirty the base workspace. The safe invariant is one base-mutating lane occupant at a time.
