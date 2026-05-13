# Design: ResolveWait dirty-clean transition

## Current Architecture

Manual merge retry in parallel/TUI mode is intended to be scheduler-owned:

1. TUI `M` on `MergeWait` records reducer-visible retry intent via `ReducerCommand::ResolveMerge`.
2. The reducer represents that intent as `WaitState::ResolveWait` and exposes it through `resolve_wait_change_ids()` and `all_display_statuses()`.
3. The scheduler promotes pending base-mutating lane waiters through reducer state and executes merge retry work.
4. Completion/deferred events feed back into the reducer as the authority for display and next action.

The base-mutating lane is single-occupant: only one non-terminal `Resolving` or `Rejecting` change may occupy it.

## Failure Mode

`ResolveWait` conflates two different situations unless the scheduler feeds concrete retry evidence back into the reducer:

- auto-resumable wait: another active base-mutating lane occupant is blocking retry
- manual wait: the base repository itself is dirty with no active resolving/rejecting owner

If dirty-base retry evidence is not applied as `MergeDeferred(auto_resumable=false)`, the row remains `resolve pending` even though it needs manual cleanup.

If base cleanliness changes from dirty to clean but no scheduler re-evaluation is triggered, the row remains `resolve pending` even though it is now retry-ready.

## Desired State Model

`ResolveWait` is a scheduler retry intent, not a permanent state.

A scheduler evaluation must classify each candidate using observable state:

- active `Resolving` or `Rejecting` for another change: keep or re-enter `ResolveWait`
- dirty base with no active base-mutating lane owner: transition to `MergeWait`
- clean base and lane free: promote exactly one candidate to `Resolving`
- terminal merged/rejected/stopped: do not resurrect retry state

`ChangesRefreshed` workspace observations may preserve or reconcile display, but they are not sufficient by themselves to demote a user retry intent from `ResolveWait` to `MergeWait`.

## Implementation Approach

### Reducer

Keep the reducer as the authority for display and queue state:

- `MergeDeferred(auto_resumable=false)` from a `ResolveWait` row must set `wait_state = MergeWait`, clear active state, clear queue intent, and remove resolve-wait queue membership.
- `promote_next_base_mutating_lane_waiter()` must set exactly one candidate to `ActivityState::Resolving` when the lane is free.
- reducer invariants must reject multiple base-mutating lane occupants.

### Scheduler

The scheduler should not rely only on resolve-wait set changes to retry. It must re-evaluate pending `ResolveWait` when base cleanliness changes or when a scheduler evaluation observes a clean, free lane.

The retry path should remain idempotent:

- repeated dirty observations should not produce unbounded logs or busy loops
- dirty-to-clean should generate one retry opportunity
- retry completion/defer events should update reducer state before TUI display sync

### TUI

TUI should continue to treat `M` as intent-only:

- pressing `M` does not directly execute merge retry
- display status comes from reducer snapshots
- local display caches must not override reducer demotion/promotion evidence

## Constitution Compliance

The proposal does not introduce durable workflow-control state outside the workspace. Runtime retry classification is derived from:

- workspace git state
- base repository git state
- base-branch comparison / merge evidence
- in-memory scheduler state for the currently running process

Logs, UI caches, and local state files remain non-authoritative observability only.

## Risks

- Retrying every scheduler tick could cause noisy logs or repeated deferred merge attempts. The implementation should dedupe unchanged dirty-base state and only re-trigger on meaningful state changes or explicit scheduler wakeups.
- Promoting more than one waiter would violate the base-mutating lane invariant. Reducer-level tests should prove exactly-one promotion.
- Treating all dirty base cases as auto-resumable would hide required manual cleanup. Dirty base with no active lane occupant must remain manual `MergeWait`.
