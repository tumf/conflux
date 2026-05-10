## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

#### Scenario: stale archive-incomplete deferral after merge success is ignored

**Given**: change `alpha` has already emitted `MergeCompleted` and is integrated into the base branch
**When**: a stale duplicate post-archive merge path emits `MergeDeferred(auto_resumable=false)` for `alpha` because the archived worktree appears dirty or incomplete during cleanup
**Then**: `alpha` remains terminal `Merged`
**And**: `alpha` is not returned by `queued_change_ids()` or `resolve_wait_change_ids()`
**And**: the user does not see a new manual `MergeWait` blocker for `alpha`

#### Scenario: real dirty-base deferral remains manual merge wait

**Given**: change `beta` is archive-complete but not integrated into the base branch
**And**: the base working tree has uncommitted changes before merge starts
**When**: the merge attempt is deferred with `auto_resumable=false`
**Then**: `beta` displays as `merge wait`
**And**: `beta` is not silently marked merged
**And**: explicit user retry remains required after the dirty-base blocker is resolved

### Requirement: Parallel Resume Applies Archive-Complete Wait Semantics

In Parallel execution mode, when a resumed workspace is already archive-complete, the shared lifecycle state SHALL apply the same wait semantics as a `ChangeArchived` transition.

This resume-time archive-complete transition MUST preserve the user-visible merge-wait lifecycle and MUST NOT fall back to `not queued` before merge handling has been attempted.

Queue reconciliation MUST NOT redispatch an archive-complete workspace as ordinary queued work while the same change already has an active post-archive merge task or repository-visible base integration.

#### Scenario: active post-archive merge suppresses duplicate archived repair dispatch

**Given**: change `gamma` has an archive-complete workspace
**And**: a post-archive merge task for `gamma` is already active
**When**: scheduler queue reconciliation scans existing worktrees
**Then**: `gamma` is not added again to the ordinary queued dispatch list
**And**: no second merge task is spawned solely from the archived dirty repair candidate path
