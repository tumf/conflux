## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker.

#### Scenario: manual dirty-base deferral clears normal queue intent

**Given**: change `alpha` has already archived in parallel mode
**And**: `alpha` still has normal queued intent from an earlier dispatch
**When**: the reducer processes `MergeDeferred` for `alpha` with `auto_resumable=false` because the base working tree has uncommitted changes
**Then**: `alpha` SHALL display as `merge wait`
**And**: `alpha` SHALL NOT be returned by `queued_change_ids()`
**And**: `alpha` SHALL NOT remain in reducer-owned resolve-wait retry membership

#### Scenario: manual merge-wait change is not re-added by queue reconciliation

**Given**: change `alpha` is in `MergeWait` after a manual merge deferral
**And**: the archived workspace for `alpha` is still present
**When**: scheduler queue reconciliation reads reducer-owned queued candidates
**Then**: `alpha` SHALL NOT be added to the scheduler-local queued list as ordinary apply/archive work
**And**: the scheduler SHALL NOT repeatedly resume the archived workspace solely because the base working tree remains dirty

#### Scenario: explicit retry after manual deferral enters resolve wait

**Given**: change `alpha` is in `MergeWait` after a manual merge deferral
**And**: the user has resolved the manual blocker outside Conflux
**When**: the user requests merge retry for `alpha` through `ResolveMerge`
**Then**: the reducer SHALL transition `alpha` to `ResolveWait`
**And**: `alpha` SHALL be returned by `resolve_wait_change_ids()`
**And**: scheduler-owned merge retry may attempt the archived workspace merge

#### Scenario: auto-resumable deferral remains scheduler retry intent

**Given**: change `beta` has a merge attempt deferred because another resolve or merge is in progress
**When**: the reducer processes `MergeDeferred` for `beta` with `auto_resumable=true`
**Then**: `beta` SHALL display as `resolve pending`
**And**: `beta` SHALL be returned by `resolve_wait_change_ids()`
**And**: `beta` SHALL remain eligible for automatic retry after the blocking merge or resolve completes
