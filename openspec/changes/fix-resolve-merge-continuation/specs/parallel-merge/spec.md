## ADDED Requirements

### Requirement: Sequential resolve retains worktree evidence

Sequential merge resolution MUST retain the ordered worktree path for every `(revision, change_id)` from merge admission through retry and terminal verification. A process-local workspace list MAY optimize path lookup but MUST NOT be the authoritative source. A stale path MAY be replaced only by repository-local Git worktree rediscovery that proves exact repository and branch identity. Missing, ambiguous, detached, mismatched, or unreadable evidence MUST fail closed and MUST NOT skip worktree verification.

#### Scenario: Archived worktree is absent from process memory

**Given**: Merge admission supplies a valid archived worktree path for a change
**And**: The process-local workspace list does not contain that worktree
**When**: Sequential resolve builds and verifies an attempt
**Then**: It uses the supplied validated path
**And**: It checks that worktree's branch, `MERGE_HEAD`, conflicts, pre-sync subject, and ancestry as applicable
**And**: It does not display `(unknown)` or fall through because process memory omitted the workspace

#### Scenario: Stale path is safely rediscovered

**Given**: A supplied archived worktree path no longer exists
**And**: Repository-local Git worktree metadata contains exactly one worktree checked out on the expected branch
**When**: Sequential resolve validates path identity
**Then**: It uses the rediscovered path
**And**: The decision remains derivable from repository-local evidence

#### Scenario: Worktree identity cannot be proven

**Given**: A path is missing, ambiguous, in another repository, on the wrong branch, detached, or fails a Git query
**When**: Sequential resolve validates evidence
**Then**: The attempt is classified as unsafe evidence
**And**: No worktree check is skipped
**And**: No blind commit or resolve-completed event occurs

### Requirement: Sequential resolve phase-specific continuation

Sequential merge resolution MUST classify the earliest incomplete or unsafe state for each change in declared merge order. The closed states MUST cover unsafe evidence, identity-verified target merge in progress, identity-verified worktree pre-sync in progress, invalid or missing pre-sync, missing final integration, archive resurrection cleanup, and complete integration. Agent exit status, narrative claims, external logs, and out-of-worktree durable state MUST NOT establish completion or choose the next phase.

#### Scenario: Pre-sync complete and final merge missing

**Given**: A validated change worktree contains required pre-sync evidence
**And**: The expected revision is not integrated into target `HEAD`
**When**: Conflux verifies a resolve attempt
**Then**: The attempt remains incomplete
**And**: Continuation identifies final merge as the next phase
**And**: Continuation names the change, branch, validated worktree path, target branch, exact `Merge change: <change_id>` subject, and cleanup requirement
**And**: Continuation does not instruct the agent to repeat pre-sync

#### Scenario: Expected worktree pre-sync remains unfinished

**Given**: The validated change worktree is on the expected branch
**And**: Its `MERGE_HEAD` is proven to contain the expected target state
**When**: Conflux verifies a resolve attempt
**Then**: Continuation identifies pre-sync completion at that worktree path
**And**: Continuation includes exact subject `Pre-sync base into <change_id>`
**And**: No resolve-completed event is emitted

#### Scenario: Merge in progress has unexpected identity

**Given**: The target repository or change worktree contains `MERGE_HEAD`
**And**: Its parent identity does not match the expected change revision or target state for the current ordered phase
**When**: Conflux classifies the state
**Then**: It reports unsafe evidence
**And**: It does not instruct the agent to commit that merge

#### Scenario: Multiple changes retain merge order

**Given**: Sequential resolve receives multiple ordered changes
**When**: More than one change is incomplete
**Then**: Classification reports the first incomplete or unsafe change in declared merge order
**And**: A target merge is never completed with a combined unauditable per-change subject

### Requirement: Sequential final integration has one identity policy

Retry verification and terminal merge verification MUST apply the same integration identity policy. A protocol-created final merge commit MUST use exact subject `Merge change: <change_id>` and integrate the expected revision. An expected revision already ancestral to target `HEAD` MUST remain an accepted idempotent already-integrated state without requiring an artificial merge commit. An exact-subject commit that does not integrate the expected revision MUST NOT prove completion.

#### Scenario: Exact final commit integrates expected revision

**Given**: Target history since the merge base contains exact subject `Merge change: <change_id>`
**And**: That commit integrates the expected revision
**When**: Retry or terminal verification runs
**Then**: Final merge identity passes

#### Scenario: Exact subject has wrong parentage

**Given**: Target history contains exact subject `Merge change: <change_id>`
**But**: That commit does not integrate the expected revision
**When**: Retry or terminal verification runs
**Then**: The evidence is rejected
**And**: The change is not reported complete

#### Scenario: Revision is already integrated without exact subject

**Given**: No exact final subject exists since the merge base
**And**: The expected revision is already an ancestor of target `HEAD`
**When**: Retry or terminal verification runs
**Then**: The change is accepted as already integrated
**And**: Conflux does not manufacture an empty merge commit

### Requirement: Archive resurrection is a terminal invariant

Sequential resolve MUST validate archive identity with the shared OpenSpec archive layout helpers and MUST prevent successful completion while the active live change and a valid archived form coexist. Before final merge, cleanup prediction MAY use valid archive evidence from the validated change worktree or branch plus target live evidence. During and after final merge, the target worktree/index-visible tree is authoritative. Invalid, nested, unrelated, or suffix-collision archive entries MUST NOT authorize removal.

#### Scenario: Final merge will resurrect a live change

**Given**: The target tree contains the active `openspec/changes/<change_id>` identity
**And**: The validated change worktree or branch contains a valid exact or date-prefixed archive entry for the same change
**When**: Continuation identifies final merge as next
**Then**: It requires removal of the resurrected active change before the final commit

#### Scenario: Final integration retains live and archive forms

**Given**: The expected revision is integrated into target `HEAD`
**And**: The target-visible tree still contains both the active change and a valid archived entry
**When**: Terminal verification runs
**Then**: Resolve remains incomplete
**And**: Continuation identifies resurrection cleanup rather than reporting success

#### Scenario: Archive layout is invalid or unrelated

**Given**: The only archive-like path is nested, unrelated, or merely suffix-similar
**When**: Sequential resolve evaluates cleanup authority
**Then**: It does not authorize live-directory removal
**And**: An invalid layout is reported as unsafe evidence

### Requirement: Sequential resolve continuation is bounded and observable

Phase diagnostics MUST be emitted through existing resolve output and retry-history surfaces without creating a second workflow state machine. Recorded attempts MUST NOT exceed configured retries. Each stdout and stderr tail MUST be capped at 2 KiB, and the complete injected resolve context MUST be capped at 8 KiB on UTF-8 boundaries while retaining the newest actionable phase diagnosis.

#### Scenario: Repeated oversized output remains bounded

**Given**: Resolve attempts emit oversized lines or echo prior prompts
**When**: Conflux records and injects continuation history
**Then**: Each stream tail is at most 2 KiB
**And**: Complete injected context is at most 8 KiB
**And**: Truncation preserves valid UTF-8 and the newest actionable phase diagnosis
**And**: Queue and resume decisions remain derived from workspace-local evidence
