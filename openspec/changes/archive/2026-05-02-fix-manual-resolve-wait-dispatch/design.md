# Design: Manual Resolve Wait Dispatch

## Current Ownership Model

Conflux already defines the intended split:

- Reducer: records lifecycle intent and derives display state.
- Scheduler: owns execution of merge / resolve retries.
- TUI: captures user intent and renders derived status.

The observed bug happens at the seam between reducer intent and scheduler execution. The TUI and reducer can agree that a change is `ResolveWait`, while the scheduler's local retry set is still empty at the moment it evaluates idle/drained state.

## Design Constraints

- Follow `openspec/CONSTITUTION.md`: do not add durable workflow-control state outside workspace/git/base-tree inputs.
- Keep scheduler execution ownership. The TUI must not directly invoke merge resolution.
- Keep `merge wait` and `resolve pending` labels as lifecycle display states.
- Avoid using string parsing of display labels or dirty reasons as control input.

## Approach

### 1. Shared intent sync point

The scheduler should sync reducer-owned resolve intent from shared `OrchestratorState` before it decides whether work is drained and before it waits as idle.

This can be implemented as a small helper in the parallel executor or queue state layer that updates the scheduler's in-memory `resolve_wait_changes` from `resolve_wait_change_ids()` when shared state is available.

### 2. Scheduler-owned retry execution

After sync, the existing retry path can operate on the scheduler-visible resolve wait set. Manual `M` key flow remains:

1. TUI `resolve_merge()` validates selected row is `merge wait`.
2. Reducer command records `ResolveWait` intent.
3. Command handler applies `ResolveMerge` and calls `notify_scheduler()`.
4. Scheduler wakes, syncs reducer-owned intent, and attempts retry through merge/retry code.

### 3. Completion semantics remain event-driven

The fix should not special-case terminal transitions in the TUI. Successful merge/retry must still emit and apply existing lifecycle events so reducer state becomes the source of truth for display.

## Risks

- Syncing shared reducer state too late can still leave the scheduler idle with pending manual resolve intent.
- Syncing one-way from reducer to scheduler must not overwrite active local retry bookkeeping in a way that loses currently executing retry work.
- Tests must distinguish manual reducer-owned resolve intent from auto-resumable deferrals caused by an active resolve/reject lane.

## Verification Strategy

- Unit tests around reducer-to-scheduler sync.
- TUI state tests proving `M` persists `ResolveWait` through refresh.
- Parallel scheduler tests proving reducer-owned intent prevents false drained/idle decisions.
- Existing merge deferred tests proving auto-resumable behavior is unchanged.
