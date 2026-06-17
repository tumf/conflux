# Design: Manual merge-wait clean retry

## Current flow

In the TUI `Changes` view, `M` enters the manual merge retry path:

1. `handle_merge_key()` calls `AppState::resolve_merge()`.
2. `resolve_merge()` gates on visible `merge wait`, updates local display to `resolve pending`, and returns `TuiCommand::ResolveMerge`.
3. `TuiCommand::ResolveMerge` applies reducer-owned retry intent and either notifies the running scheduler or starts a scheduler run with no normal queued changes.
4. Scheduler/executor retry dispatch consumes reducer-owned `ResolveWait` and attempts post-archive merge handling.

The observed bug indicates that after a dirty manual deferral returns the row to `merge wait`, a later clean `M` does not produce a consumed scheduler retry.

## Design principles

- The shared reducer is the source of truth for retry intent.
- TUI display may be optimistic only while it matches reducer-owned retry membership or immediately rolls back with visible blocker evidence.
- Manual retry after `MergeDeferred(auto_resumable=false)` is a new user intent and must invalidate stale retry dispatch suppression for that change.
- Scheduler startup with no normal queued changes is valid when reducer-owned base-lane retry work exists.
- Workflow-control decisions must remain derivable from workspace file state, workspace git state, and base-branch comparison.

## Implementation approach

### TUI/reducer boundary

Avoid treating a local display transition to `resolve pending` as proof that retry work exists. The command handler should be able to distinguish:

- accepted fresh retry intent;
- already-pending retry intent that is still scheduler-consumable;
- rejected stale input that must roll the row back or warn.

If `resolve_merge()` continues to pre-apply reducer intent for refresh-stability, command handling must treat the second idempotent reducer application as accepted pending work rather than suppressing scheduler startup solely because the reducer was already moved to `ResolveWait` by the same keypress.

### Scheduler dispatch

When a manual retry intent is accepted and no scheduler is alive, the zero-normal-queue scheduler run must check and consume reducer-owned resolve waiters before deciding that execution completed with zero changes.

When a scheduler is alive, notification must wake base-lane waiter dispatch even if no ordinary queued apply candidates exist.

### Stale retry dedupe

Dirty manual deferral must clear or invalidate per-change dispatch snapshots so that a later explicit retry is evaluated against current workspace/base state. Dedupe may prevent repeated automatic retry churn, but must not suppress an explicit user retry after `merge wait`.

## Verification strategy

Tests must prove behavior through reducer membership and scheduler dispatch effects, not only through visible `display_status_cache` strings.

Recommended regression shape:

1. Start with change `alpha` in reducer-visible `MergeWait` and archive-complete workspace evidence.
2. Press/dispatch `ResolveMerge(alpha)` while dirty preconditions are simulated.
3. Assert `alpha` returns to `MergeWait`, `ResolveWait(alpha)` is cleared, and no normal queue intent appears.
4. Clear dirty preconditions.
5. Press/dispatch `ResolveMerge(alpha)` again.
6. Assert reducer records `ResolveWait(alpha)` and scheduler dispatch reaches the merge attempt path.
7. Assert success can emit merge completion and derive `merged`.

## Risks and mitigations

- Risk: making `ResolveMerge` idempotent could revive stale terminal work.
  - Mitigation: preserve terminal-state guards for `Merged`, `Rejected`, and unretryable terminal states.
- Risk: scheduler zero-queue startup may look like ordinary empty execution.
  - Mitigation: classify the run as manual retry work whenever reducer-owned resolve waiters exist.
- Risk: fixing this in TUI display only would mask the bug.
  - Mitigation: require tests around reducer-owned membership and scheduler dispatch consumption.
