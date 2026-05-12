# Design: Post-Archive False Merge Wait Regression

## Premise

The observed symptom is a false state vibration: archive completion is followed by a short `merge wait` display before the same change proceeds to `resolving`. The intended state machine does not use `merge wait` as a transient archive-complete milestone. `merge wait` means manual intervention is required.

This crosses multiple layers:

1. Parallel execution emits archive/merge/deferred events.
2. The shared reducer derives authoritative lifecycle display status.
3. The TUI runner applies reducer snapshots and then handles local refresh events.
4. Periodic refresh derives `merge_wait_ids` from workspace/archive evidence.
5. Web state applies reducer-derived queue status after refresh updates.

Because of this layering, a local display fix can mask the symptom without repairing the state-machine contract. The implementation must first prove which boundary produces the false `merge wait`.

## State Semantics

### `resolving`

`resolving` means post-archive base-mutating work for the change is active or can proceed immediately. A bare archive-complete workspace is sufficient to require merge handling, but it is not sufficient to prove manual wait.

### `resolve pending`

`resolve pending` means reducer-owned scheduler retry intent exists but another base-mutating lane occupant currently blocks execution. It is auto-resumable scheduler work.

### `merge wait`

`merge wait` means manual action is required. It must be backed by concrete manual deferral evidence such as `MergeDeferred(auto_resumable=false)` or an equivalent repository/workspace classification. It must not be created merely because a workspace is archived but not yet merged.

## Source Precedence

When multiple observations disagree, state precedence is:

1. Terminal reducer state (`merged`, `rejected`, `error`, `archived` where applicable)
2. Active reducer activity (`resolving`, `rejecting`, `archiving`, etc.)
3. Reducer-owned wait states (`resolve pending`, `reject pending`, `merge wait`)
4. Queue intent
5. Refresh-derived display hints

Refresh-derived `merge_wait_ids` can correct stale local display, but cannot override items 1-3.

## Suspected Regression Path

The likely path is:

1. `ChangeArchived` updates reducer state to `resolving` when no other lane blocker exists.
2. TUI displays reducer-derived `resolving`.
3. Periodic refresh detects `WorkspaceState::Archived` and emits the change in `merge_wait_ids`.
4. TUI refresh handler applies local `merge wait` because it only protects terminal rows and reducer-owned `resolve pending`.
5. A later reducer/merge event updates the row back to `resolving` or `merged`.

This explains a visible few-second false `merge wait` without requiring the reducer itself to be in `MergeWait`.

## Implementation Approach

1. Add regression tests that encode the suspected event ordering before changing behavior.
2. Fix reducer observation semantics only if the reducer can still regress internally.
3. Fix TUI refresh precedence so reducer-owned active states cannot be downgraded by refresh hints.
4. Audit parallel merge status events so auto-resumable deferrals do not publish manual wait evidence.
5. Confirm Web state remains reducer-derived and does not mirror stale TUI display hints.

## Risks and Mitigations

- Risk: Blocking stale-display correction entirely would reintroduce rows stuck at local `resolve pending`.
  - Mitigation: preserve correction only when the reducer snapshot does not own active/pending/terminal state.

- Risk: Treating all archived workspaces as `resolving` could hide true manual blockers.
  - Mitigation: keep `MergeDeferred(auto_resumable=false)` and explicit manual blocker classification as the only path to `merge wait`.

- Risk: Parallel scheduler internal sets (`resolve_wait_changes`, `merge_wait_changes`) diverge from reducer-owned state.
  - Mitigation: tests must assert both reducer display and scheduler retry membership for auto-resumable and manual deferrals.

- Risk: UI display state becomes workflow-control input.
  - Mitigation: comply with `openspec/CONSTITUTION.md`; display caches remain observability only.
