## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker. If repository-visible evidence still shows an archived-but-not-merged change is waiting for manual merge retry, the reducer MUST accept retry intent in a form that remains scheduler-consumable and MUST NOT silently drop it while the TUI continues to display pending retry.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

The shared reducer state that accepts `ResolveMerge` MUST be the same authoritative reducer state observed by the scheduler/executor that consumes the retry. A service or executor construction path MUST NOT replace caller-owned reducer state with a fresh empty reducer state after retry intent has been accepted. State synchronization may copy reducer-owned lane-wait membership into executor-local caches, but the copied cache MUST NOT become an independent source of truth that can make the UI show `resolve pending` after reducer-owned membership has been cleared.

<!-- Expected canonical result after archive: `orchestration-state` will make reducer state ownership explicit so manual retry cannot be accepted in one reducer and consumed from another empty reducer. -->

#### Scenario: archived merge-wait manual retry becomes reducer-owned resolve wait

**Given**: change `alpha` is archive-complete and not yet merged into the base branch
**And**: repository-visible evidence requires manual merge retry rather than ordinary queued apply work
**When**: an explicit `ResolveMerge(alpha)` command is applied
**Then**: the reducer records `alpha` in `ResolveWait`
**And**: `alpha` is returned by reducer-owned resolve-wait membership for scheduler retry
**And**: the command does not become `NoOp` solely because `alpha` was previously archive-complete

#### Scenario: merged change still rejects stale manual retry intent

**Given**: change `alpha` is already repository-visible `Merged`
**When**: a stale explicit `ResolveMerge(alpha)` command is applied
**Then**: the reducer rejects that retry intent
**And**: `alpha` is not reintroduced into resolve-wait membership
**And**: later refreshes do not regress `alpha` to `MergeWait` or `ResolveWait`

#### Scenario: manual retry scheduler uses the accepting reducer

**Given**: the TUI shared reducer accepts `ResolveMerge(alpha)` and records `ResolveWait(alpha)`
**And**: the scheduler was not running, so the TUI starts a manual resolve scheduler with no normal queued changes
**When**: the parallel run service constructs or configures the scheduler executor
**Then**: the executor observes `ResolveWait(alpha)` from the same shared reducer state that accepted the command
**And**: no construction path replaces that shared reducer with an empty reducer before retry evaluation

#### Scenario: executor-local cache cannot outlive reducer truth

**Given**: executor-local retry sets were synchronized from shared reducer state
**When**: reducer events clear `ResolveWait(alpha)` by merge completion, manual deferral, rejection, or stale-prerequisite handling
**Then**: executor-local retry sets and TUI display are reconciled to the reducer-cleared state
**And**: `alpha` is not left visible as `resolve pending` solely because an executor-local cache still contained `alpha`
