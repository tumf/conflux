## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When the scheduler retries an archived Git merge and the merge path reaches a normal merge-ready state without unresolved conflicts, the runtime SHALL complete that merge through the normal merge/verification path and SHALL NOT start AI conflict resolution solely because the retry entered the resolve-capable code path.

Post-merge verification for this path SHALL accept repository-visible merge success without requiring the archived source branch tip to continue containing the pre-merge base after the merge commit has already integrated the change into the target branch.

When a reducer-owned deferred merge retry succeeds, the runtime SHALL remove that change from retry intent before a later scheduler sync can reintroduce it. Successful merge completion SHALL be idempotent with respect to repeated retry triggers for the same change.

#### Scenario: Conflictless archived merge does not emit resolve command

**Given**: change `alpha` is archived and reaches scheduler-owned merge retry
**And**: the target branch merge preparation succeeds without unresolved conflicts
**And**: conflict detection returns no conflict files
**When**: the runtime evaluates whether to start conflict resolution
**Then**: it SHALL NOT emit `ResolveStarted` for `alpha`
**And**: it SHALL NOT build a conflict-oriented resolve prompt for `alpha`
**And**: it SHALL continue through the normal merge completion path

#### Scenario: Successful merge commit is not retried for false pre-sync negative

**Given**: change `alpha` is archived and merged into the target branch by a valid merge commit
**And**: the archived source branch tip itself no longer proves inclusion of the pre-merge base
**When**: post-merge verification runs
**Then**: the runtime SHALL accept the merged outcome from repository-visible merge evidence
**And**: it SHALL NOT retry resolve solely because the source branch tip does not include the pre-merge base

#### Scenario: True conflict still enters resolve path

**Given**: change `alpha` is archived and reaches scheduler-owned merge retry
**And**: the target branch merge preparation leaves unresolved conflicts
**When**: the runtime evaluates conflict resolution
**Then**: it SHALL emit `ResolveStarted` for `alpha`
**And**: the resolve prompt SHALL include non-empty conflict evidence

#### Scenario: successful deferred retry clears retry intent

**Given**: change `alpha` is in reducer-owned resolve-wait retry intent
**And**: the scheduler retries and completes the merge successfully
**When**: scheduler-local retry state is synchronized from reducer-owned state again
**Then**: `alpha` is not returned as a resolve-wait retry candidate
**And**: the scheduler does not attempt another merge for `alpha`

#### Scenario: stale retry for merged change is consumed without side effects

**Given**: change `alpha` is already repository-visible as merged into the base branch
**And**: stale in-memory retry state still contains `alpha`
**When**: deferred merge retry processing evaluates `alpha`
**Then**: the stale retry entry is removed
**And**: no merge command, resolve command, or merge hook is executed for `alpha`
