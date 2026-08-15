## MODIFIED Requirements

### Requirement: Target merge shortcuts obey full verification

A conflict-free target `MERGE_HEAD` MUST NOT be committed before batch ownership, required target state, pre-sync topology, index conflict stages, resurrection cleanup, and terminal predicates are evaluated. Resolve MUST NOT generate a combined `Merge changes: ...` commit for per-change sequential integration.

An explicit manual resolve retry for a change in `merge wait` MUST be allowed to enter repository-derived sequential classification while target `MERGE_HEAD` exists. For that admitted retry, and only while target `MERGE_HEAD` exists, the generic base-dirty preflight SHALL be superseded for one dispatch by a scoped evidence check: target `MERGE_HEAD` MUST belong uniquely to the selected change, every deviation from `HEAD` MUST be attributable to the in-progress merge and its staged result, and the working tree MUST match the index. Unstaged modifications, staged content not produced by the merge, or conflicting untracked paths MUST fail closed without mutation before any agent invocation.

For every other merge attempt the generic preflight remains authoritative and unchanged. The scoped authorization MUST be change-bound, consumed by the single admitted dispatch, and MUST NOT become durable workflow state. Resolve/base-mutation occupancy remains higher-priority evidence: when another operation owns the lane, the retry MUST remain auto-resumable and MUST NOT consume the authorization. All identity, topology, conflict, cleanup, and terminal checks remain mandatory after admission.

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

#### Scenario: Manual retry rejects unrelated dirt alongside its own merge

**Given**: Target `MERGE_HEAD` uniquely matches change `alpha`
**And**: The target also contains an unrelated staged change, unstaged modification, or conflicting untracked path
**When**: The operator presses `M`
**Then**: The scoped evidence check rejects the retry before any agent invocation
**And**: It does not commit, abort, clean, reset, stage, or discard repository state

#### Scenario: Occupied base lane preserves retry authorization

**Given**: Change `alpha` has an admitted manual retry
**And**: Another resolve or base mutation owns the lane
**When**: The retry is dispatched
**Then**: It is deferred with `auto_resumable=true`
**And**: Its one-dispatch scoped authorization is not consumed

#### Scenario: Scoped authorization is not sticky

**Given**: A manual retry dispatch for change `alpha` consumed its scoped authorization
**When**: A later ordinary scheduled attempt observes a dirty target
**Then**: The unchanged generic dirty preflight applies
**And**: The scheduled attempt does not enter sequential resolve classification
