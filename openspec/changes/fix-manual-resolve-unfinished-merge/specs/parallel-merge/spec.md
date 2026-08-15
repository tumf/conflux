## MODIFIED Requirements

### Requirement: Target merge shortcuts obey full verification

A conflict-free target `MERGE_HEAD` MUST NOT be committed before batch ownership, required target state, pre-sync topology, index conflict stages, resurrection cleanup, and terminal predicates are evaluated. Resolve MUST NOT generate a combined `Merge changes: ...` commit for per-change sequential integration.

An explicit manual resolve retry for a change in `merge wait` MUST be allowed to enter repository-derived sequential classification while target `MERGE_HEAD` exists. The generic base-dirty preflight MUST NOT reject that retry solely because of the unfinished merge it is intended to continue. This exception applies only after retry intent is admitted and exclusive base-lane ownership is acquired; all identity, topology, conflict, cleanup, and terminal checks remain mandatory.

<!-- Expected canonical result after archive: manual retry can safely continue its own verified unfinished target merge, while foreign or invalid dirty state remains fail-closed. -->

#### Scenario: Conflict-free target merge is valid

**Given**: Target `MERGE_HEAD` uniquely owns the first incomplete item
**And**: Pre-sync, index, and cleanup evidence are valid
**When**: Resolve continues the phase
**Then**: The agent receives the exact per-change final subject and required actions
**And**: Completion is reverified after commit

#### Scenario: Manual retry continues its unfinished target merge

**Given**: Bounded resolve retries exhausted for change `alpha`
**And**: The target retains a conflict-free `MERGE_HEAD` uniquely matching `alpha`'s validated branch tip
**And**: `alpha` is visible as manual `merge wait`
**When**: The operator presses `M` and retry intent acquires the base lane
**Then**: The existing `MERGE_HEAD` does not cause a generic dirty-state deferral
**And**: Sequential resolve classifies and continues the repository-derived phase
**And**: Successful completion is committed as `Merge change: alpha` and reverified

#### Scenario: Manual retry rejects foreign or unsafe dirty state

**Given**: Change `alpha` is visible as manual `merge wait`
**But**: Target `MERGE_HEAD` is foreign or ambiguous, conflicts remain, topology is invalid, evidence is unreadable, or unrelated dirty changes exist
**When**: The operator presses `M`
**Then**: Resolve fails closed with actionable evidence
**And**: It does not commit, abort, clean, reset, or discard repository state
