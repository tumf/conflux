## ADDED Requirements

### Requirement: Sequential resolve phase-specific continuation

Sequential merge resolution MUST derive retry continuation from current repository and worktree evidence and MUST identify the earliest unfinished protocol phase for each change. Continuation MUST distinguish target-repository merge completion, worktree pre-sync completion, pre-sync evidence validation, final merge completion, and archive resurrection cleanup. Agent exit status, narrative claims, external logs, and out-of-worktree durable state MUST NOT establish completion or choose the next phase.

#### Scenario: Pre-sync complete and final merge missing

**Given**: A change worktree has completed the required pre-sync with the target branch
**And**: The target branch does not contain the change branch or an exact `Merge change: <change_id>` commit
**When**: Conflux verifies a resolve attempt
**Then**: The attempt remains incomplete
**And**: Continuation identifies final merge as the next phase
**And**: Continuation names the change ID, branch, worktree path, target branch, and exact final merge subject
**And**: Continuation does not instruct the agent to repeat the completed pre-sync

#### Scenario: Worktree pre-sync merge remains unfinished

**Given**: The change worktree contains `MERGE_HEAD` or unresolved conflicts from pre-sync
**When**: Conflux verifies a resolve attempt
**Then**: The attempt remains incomplete
**And**: Continuation identifies the worktree path and pre-sync completion as the next phase
**And**: Continuation includes the exact `Pre-sync base into <change_id>` subject
**And**: No resolve-completed event is emitted

#### Scenario: Resurrection cleanup is required before final commit

**Given**: The target tree contains `openspec/changes/<change_id>`
**And**: `openspec/changes/archive` contains a valid exact or date-prefixed archive entry for the same change
**And**: The final merge is not complete
**When**: Conflux builds continuation for the final merge phase
**Then**: Continuation requires removal of the resurrected live change directory before the final merge commit
**And**: An unrelated or invalid archive entry does not authorize removal

#### Scenario: Final integration is repository-verifiable

**Given**: No target-repository or worktree merge is unfinished
**And**: No unresolved conflict remains
**And**: Required pre-sync subject and ancestry checks pass
**And**: The target branch contains the integrated revision with the required final merge evidence
**When**: Conflux verifies the resolve attempt
**Then**: The resolve may complete
**And**: Completion does not depend on the resolve command's narrative output

### Requirement: Sequential resolve continuation is bounded and observable

Phase-specific continuation diagnostics MUST be bounded, stable for equivalent repository state, and emitted through the existing resolve output and retry-history surfaces. They MUST NOT create a second workflow state machine or persist authoritative continuation outside the workspace.

#### Scenario: Equivalent retries receive stable bounded guidance

**Given**: Two consecutive resolve attempts leave equivalent Git and OpenSpec tree state
**When**: Conflux records their continuation diagnostics
**Then**: Both diagnostics identify the same unfinished phase and required action
**And**: Diagnostic size remains bounded rather than recursively embedding unbounded prior history
**And**: Queue and resume decisions remain derived from workspace-local evidence
