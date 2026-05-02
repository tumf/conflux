## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When the scheduler retries an archived Git merge and the merge path reaches a normal merge-ready state without unresolved conflicts, the runtime SHALL complete that merge through the normal merge/verification path and SHALL NOT start AI conflict resolution solely because the retry entered the resolve-capable code path.

Post-merge verification for this path SHALL accept repository-visible merge success without requiring the archived source branch tip to continue containing the pre-merge base after the merge commit has already integrated the change into the target branch.

Reducer-owned `ResolveWait` SHALL be considered schedulable work even when there are no queued active changes. Scheduler startup and idle/drained checks MUST include this reducer-owned work before deciding that a run has no work.

#### Scenario: reducer-owned resolve wait survives empty startup

**Given**: change `alpha` is stored in the shared reducer as `ResolveWait`
**And**: the scheduler starts with an empty active change list
**When**: the scheduler evaluates whether work is drained
**Then**: it treats `alpha` as pending scheduler-owned retry work
**And**: it does not emit only a zero-change completion without attempting the retry

#### Scenario: resolve wait is synchronized before drained exit

**Given**: shared reducer state contains one or more `ResolveWait` changes
**When**: the scheduler loop begins an iteration
**Then**: it synchronizes those IDs into executor retry state before checking whether queued, in-flight, resolve-wait, manual-resolve, and pending-merge work are all empty
