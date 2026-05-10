## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker. If repository-visible evidence still shows an archived-but-not-merged change is waiting for manual merge retry, the reducer MUST accept retry intent in a form that remains scheduler-consumable and MUST NOT silently drop it while the TUI continues to display pending retry.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

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
