# Design: Restrict Resolve Pending Blockers

## Current Behavior

`ResolveWait` is currently used for several related but distinct concepts:

- queued/manual resolve intent for a `MergeWait` row
- archived changes waiting while another resolve is active
- auto-resumable merge deferrals emitted by post-archive handling

Because some paths mark post-archive deferrals as `auto_resumable=true` broadly, `resolve pending` can appear while unrelated lifecycle states such as applying or accepting are still running.

## Target Model

`ResolveWait` remains valid, but automatic creation is limited to a structural blocker predicate:

```text
other change is active and non-terminal and activity is Resolving or Rejecting
```

This predicate should be implemented as reducer-owned logic, exposed through a helper rather than duplicated string/status comparisons in UI code.

## State Responsibilities

- Reducer owns lifecycle truth and display derivation.
- TUI may request intent but must not independently widen the `ResolveWait` blocker set.
- Parallel scheduler owns execution/retry of pending resolve work.
- Free-form reason strings are logs only and must not be parsed for control flow.

## Rejection Interaction

If active `Rejecting` can block merge/resolve and produce `ResolveWait`, the scheduler needs a retry trigger when rejection review ends. Completion and failure paths should notify the same retry mechanism used after merge/resolve completion, without adding durable out-of-worktree workflow state.

## Non-Goals

This proposal does not change how merge conflicts are resolved, how dirty-base manual wait is represented, or how users explicitly queue a `MergeWait` row with the TUI `M` key.
