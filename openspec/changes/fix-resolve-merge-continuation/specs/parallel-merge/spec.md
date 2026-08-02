## ADDED Requirements

### Requirement: Sequential resolve retains ordered worktree evidence

Sequential resolve MUST retain an ordered record for every admitted change containing its expected revision, change ID, archive worktree path, and any available admission-time branch base. Process-local workspace membership and workspace metadata MUST NOT be authoritative. A stale path MAY be replaced only by exactly one repository-local Git worktree match for the expected branch. Missing, ambiguous, detached, mismatched, or unreadable identity MUST fail closed.

#### Scenario: Process memory omits a valid archived worktree

**Given**: Merge admission supplied a valid registered worktree path and expected branch
**And**: The process-local workspace list omits it
**When**: Resolve classifies the batch
**Then**: It validates and uses the supplied path
**And**: It does not display `(unknown)` or skip worktree evidence

#### Scenario: Stale path has one exact repository-local match

**Given**: The supplied path is stale
**And**: Git worktree metadata has exactly one non-detached worktree on the expected branch
**When**: Resolve validates worktree identity
**Then**: It uses the rediscovered path

#### Scenario: Worktree identity is unsafe

**Given**: Worktree identity is missing, ambiguous, detached, in another repository, on another branch, or unreadable
**When**: Resolve validates the batch
**Then**: It reports unsafe evidence
**And**: It performs no blind commit and emits no completion

### Requirement: Sequential pre-sync is repository-verifiable

For each non-historical batch item, resolve MUST derive required target state `T` from repository evidence. `T` MUST be the target pre-merge first parent for an in-progress or committed exact final merge, or current cumulative target `HEAD` after every prior item is committed complete when final merge has not started. Pre-sync is valid without a merge commit only when `T` is on the validated worktree tip's first-parent lineage. Otherwise exactly one reachable `Pre-sync base into <change_id>` commit MUST have exactly two parents and non-first parent exactly `T`.

#### Scenario: Target state is already on first-parent lineage

**Given**: Required target state `T` is on the validated worktree tip's first-parent lineage
**When**: Resolve validates pre-sync
**Then**: No pre-sync merge commit is required

#### Scenario: Valid pre-sync merge includes target state

**Given**: `T` is not on the worktree tip's first-parent lineage
**And**: Exactly one reachable `Pre-sync base into <change_id>` commit has two parents and non-first parent exactly `T`
**When**: Resolve validates pre-sync
**Then**: Pre-sync is valid

#### Scenario: Pre-sync topology is invalid

**Given**: Required pre-sync evidence is missing, duplicated, has a wrong parent count, has a wrong non-first parent, or is not contained by the worktree tip
**When**: Resolve validates pre-sync
**Then**: It reports invalid pre-sync
**And**: Final merge guidance is withheld

#### Scenario: Historical integration has no reconstructable target state

**Given**: No exact final-subject candidate exists
**And**: The expected branch tip is already ancestral to target `HEAD`
**When**: Resolve validates historical integration
**Then**: It does not require reconstruction of pre-sync topology
**And**: Clean target and archive/live terminal invariants still apply

### Requirement: Sequential resolve is batch-aware

Resolve MUST uniquely determine any global target `MERGE_HEAD` owner before per-item classification. The owner MUST match exactly one validated batch branch tip, every prior item MUST have committed completion evidence, and the owner MUST be the first incomplete item. Items MUST otherwise be evaluated in declared order. Batch completion MUST require every item complete, no merge in progress, no conflict, and a clean target index and worktree including no untracked files.

#### Scenario: Later item owns target merge after prior completion

**Given**: Item A has committed completion evidence
**And**: Target `MERGE_HEAD` exactly matches item B's validated branch tip
**And**: B is the first incomplete item
**When**: Resolve classifies the batch
**Then**: B is the unique target merge owner
**And**: B's pre-sync is validated against target `HEAD` before commit guidance

#### Scenario: Target merge owner is unsafe

**Given**: Target `MERGE_HEAD` matches zero or multiple batch items, has an incomplete prior item, or does not own the first incomplete item
**When**: Resolve classifies the batch
**Then**: It reports unsafe evidence
**And**: It does not commit the target merge

#### Scenario: Batch repository is dirty

**Given**: Every item has integration evidence
**But**: Target has `MERGE_HEAD`, conflicts, staged, unstaged, or untracked changes
**When**: Resolve evaluates batch completion
**Then**: The batch is not complete

### Requirement: Final merge identity is exact or historical

Retry and terminal verification MUST share one final integration policy. An exact `Merge change: <change_id>` candidate since `base_revision` is valid only when it is unique, has exactly two parents, has first parent exactly required target state `T`, has non-first parent exactly the validated worktree branch tip, and is contained by target `HEAD`. If any exact candidate exists but is ambiguous or invalid, verification MUST fail closed. Ancestry-only historical success is allowed only when no exact candidate exists.

#### Scenario: Exact final merge has valid topology

**Given**: Exactly one exact final candidate exists
**And**: Its two parents are `T` followed by the validated worktree branch tip
**And**: Target `HEAD` contains it
**When**: Final integration is verified
**Then**: Final merge identity is valid

#### Scenario: Exact subject hides unrelated merge

**Given**: An exact final candidate has the expected revision only on its first-parent side, has a wrong non-first parent, wrong parent count, or duplicate exact candidate
**When**: Final integration is verified
**Then**: Verification fails closed
**And**: Ancestry fallback is not used

#### Scenario: No exact candidate but revision is already integrated

**Given**: No exact final candidate exists
**And**: The expected branch tip is ancestral to target `HEAD`
**When**: Final integration is verified
**Then**: It is accepted as historical already-integrated evidence
**And**: No artificial merge commit is required

### Requirement: Archive resurrection uses phase-specific Git evidence

Archive identity MUST apply shared exact/date-prefixed and invalid nested-layout rules to the appropriate Git view. Pre-final evidence MUST use committed validated worktree `HEAD` plus committed target `HEAD`; in-progress final evidence MUST use target stage-0 index and reject any conflict stage; post-final evidence MUST use committed target `HEAD`. Active and archived identities require `proposal.md`. Filesystem state MUST NOT substitute for index or committed-tree evidence.

#### Scenario: Final merge index predicts resurrection

**Given**: Target has an identity-verified final merge in progress
**And**: Its stage-0 index contains active and valid archived identities for the change
**And**: No conflict stages exist
**When**: Resolve classifies the merge
**Then**: Continuation requires live-directory removal before final commit

#### Scenario: Conflict stages prevent cleanup guidance

**Given**: Target index contains stage 1, 2, or 3 entries for relevant paths
**When**: Resolve evaluates resurrection
**Then**: It reports unsafe or unresolved conflict evidence
**And**: It does not authorize deletion

#### Scenario: Invalid archive shape cannot authorize deletion

**Given**: Archive-like evidence is nested, unrelated, suffix-similar, or lacks the archived proposal identity
**When**: Resolve evaluates cleanup authority
**Then**: It does not authorize active live removal

### Requirement: Post-final resurrection cleanup is durable

If committed final integration retains active and valid archived identities, resolve MUST require a forward commit with exact subject `Cleanup resurrected change: <change_id>`. The commit MUST have one parent equal to the preceding target `HEAD`, and its complete tree diff MUST only delete the active live change subtree while preserving archived content. Staged-only, unstaged, mixed, unrelated, amend-based, or dirty cleanup MUST remain incomplete.

#### Scenario: Cleanup is only staged

**Given**: Final integration is committed with live/archive coexistence
**And**: The active live subtree is removed only from the target index or worktree
**When**: Resolve verifies completion
**Then**: Cleanup remains incomplete

#### Scenario: Valid forward cleanup is committed

**Given**: A one-parent `Cleanup resurrected change: <change_id>` commit follows target `HEAD`
**And**: Its only tree change deletes the active live subtree
**And**: The valid archive is unchanged
**And**: Target index and worktree are clean
**When**: Resolve reruns terminal verification
**Then**: Resurrection cleanup is complete

#### Scenario: Cleanup commit changes unrelated content

**Given**: A cleanup-subject commit changes archive or unrelated paths, has wrong parentage, or is one of multiple ambiguous candidates
**When**: Resolve verifies cleanup
**Then**: It fails closed

### Requirement: Target merge shortcuts obey full verification

A conflict-free target `MERGE_HEAD` MUST NOT be committed before batch ownership, required target state, pre-sync topology, index conflict stages, resurrection cleanup, and terminal predicates are evaluated. Resolve MUST NOT generate a combined `Merge changes: ...` commit for per-change sequential integration.

#### Scenario: Conflict-free target merge is valid

**Given**: Target `MERGE_HEAD` uniquely owns the first incomplete item
**And**: Pre-sync, index, and cleanup evidence are valid
**When**: Resolve continues the phase
**Then**: The agent receives the exact per-change final subject and required actions
**And**: Completion is reverified after commit

### Requirement: Resolve continuation history is byte-bounded

Resolve continuation construction MUST cap each stdout/stderr tail at 2 KiB and the complete wrapper-inclusive `<resolve_context>` at 8 KiB on UTF-8 boundaries without changing shared collector defaults for other workflows. It MUST retain at most configured retries and MUST always retain the newest structured phase diagnosis. Deterministic trimming MUST remove oldest attempts, then older stream tails, then newest stream detail.

#### Scenario: Repeated output exceeds resolve limits

**Given**: Attempts emit oversized ASCII or multibyte output and echo prior prompts
**When**: Resolve constructs continuation context
**Then**: Each stream tail is at most 2 KiB
**And**: Complete wrapper-inclusive context is at most 8 KiB
**And**: UTF-8 remains valid
**And**: The newest structured phase diagnosis remains present

#### Scenario: Diagnosis alone approaches the context limit

**Given**: Structured diagnostic fields are oversized
**When**: Resolve constructs the newest diagnosis
**Then**: Individual fields are bounded before assembly
**And**: Wrapper-inclusive context still satisfies the 8 KiB limit
