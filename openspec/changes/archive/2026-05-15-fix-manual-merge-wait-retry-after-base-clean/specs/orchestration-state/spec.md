## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker. If repository-visible evidence still shows an archived-but-not-merged change is waiting for manual merge retry, the reducer MUST accept retry intent in a form that remains scheduler-consumable and MUST NOT silently drop it while the TUI continues to display pending retry.

After a manual `MergeDeferred(auto_resumable=false)` returns a change to `MergeWait`, a later explicit `ResolveMerge` for the same change MUST be treated as a fresh user retry intent. Executor-local retry dedupe, dirty-state tracking, or previous dispatch snapshots MUST NOT suppress this retry after the manual blocker has been resolved.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

The shared reducer state that accepts `ResolveMerge` MUST be the same authoritative reducer state observed by the scheduler/executor that consumes the retry. A service or executor construction path MUST NOT replace caller-owned reducer state with a fresh empty reducer state after retry intent has been accepted. State synchronization may copy reducer-owned lane-wait membership into executor-local caches, but the copied cache MUST NOT become an independent source of truth that can make the UI show `resolve pending` after reducer-owned membership has been cleared.

<!-- Expected canonical result after archive: `orchestration-state` will require explicit manual retry after `MergeDeferred(auto_resumable=false)` to invalidate stale scheduler dedupe and be consumed by the scheduler once workspace/git/base evidence allows it. -->

#### Scenario: dirty base manual retry returns to merge wait

**Given**: change `alpha` is archive-complete and ready for post-archive merge handling
**And**: the base repository has uncommitted changes
**When**: the user requests merge retry for `alpha`
**Then**: the merge attempt is deferred with `MergeDeferred(alpha, auto_resumable=false)`
**And**: reducer-visible status for `alpha` becomes `merge wait`
**And**: `alpha` is removed from reducer-owned resolve-wait membership
**And**: `alpha` is not reintroduced as ordinary queued apply work

#### Scenario: explicit retry after base clean starts scheduler-consumed retry

**Given**: change `alpha` is in reducer-visible `merge wait` due to dirty-base manual deferral
**And**: `alpha` still has an archive-complete workspace that is not merged to the base branch
**And**: the base repository has become clean
**When**: the user requests merge retry for `alpha` again
**Then**: `ReducerCommand::ResolveMerge(alpha)` is accepted
**And**: the reducer records `alpha` in `ResolveWait`
**And**: the active or newly-started scheduler consumes that same reducer-owned `ResolveWait` intent
**And**: retry evaluation reaches the merge attempt path for `alpha`
**And**: if no blocker remains, `alpha` can transition to `merged`

#### Scenario: live scheduler notification consumes lane waiter without ordinary queue work

**Given**: the TUI logs `Scheduled merge-wait retry intent for 'alpha'; notified existing scheduler`
**And**: the scheduler is already running
**And**: there are no ordinary queued apply candidates remaining
**When**: reducer-owned `ResolveWait(alpha)` exists
**Then**: the scheduler wakes and evaluates base-lane waiters
**And**: the retry does not require another queued change or another user keypress to make progress

#### Scenario: explicit manual retry is not suppressed by stale dispatch dedupe

**Given**: change `alpha` previously had a `ResolveWait` retry dispatched
**And**: that retry returned to manual `merge wait` through `MergeDeferred(alpha, auto_resumable=false)`
**When**: the user resolves the manual blocker and requests merge retry for `alpha` again
**Then**: stale executor-local dispatch snapshots or dirty-state observations do not suppress retry dispatch
**And**: the retry is evaluated against current workspace and base repository state

#### Scenario: accepted retry emits evidence when still blocked

**Given**: explicit retry for `alpha` is accepted into reducer-owned `ResolveWait`
**And**: retry evaluation still cannot start or complete merge handling
**When**: the scheduler evaluates the retry
**Then**: the system emits log or event evidence identifying the remaining blocker
**And**: `alpha` remains in the correct reducer-visible wait state for that blocker
