## MODIFIED Requirements

### Requirement: Resumed Archived Workspaces Preserve Merge Handoff

When parallel execution resumes a workspace already detected as `WorkspaceState::Archived`, the executor SHALL treat that workspace as archive-complete for downstream lifecycle handling.

The resumed workspace MUST NOT silently complete in a way that bypasses merge handling or causes the change to regress to `NotQueued` before merge resolution is attempted.

Queue reconciliation MUST NOT rediscover an archived or archived-dirty worktree as scheduler-local queued work when workspace-local Git/base-branch evidence shows that the same change is already merged into the base branch.

#### Scenario: Resumed archived workspace enters merge wait on restart

- **GIVEN** a parallel worktree is reused on restart
- **AND** `detect_workspace_state(change_id, workspace_path, base_branch)` returns `WorkspaceState::Archived`
- **AND** the change is not yet merged into the base branch
- **WHEN** the resumed change is dispatched
- **THEN** apply and archive are not re-run
- **AND** the resumed change is handed off to the same archive-complete lifecycle used by a freshly archived change
- **AND** the change transitions to merge handling or `MergeWait`, not `NotQueued`

#### Scenario: Resumed archived workspace participates in merge-deferred flow

- **GIVEN** a reused worktree is already `WorkspaceState::Archived`
- **AND** merge cannot proceed immediately
- **WHEN** the resumed change completes dispatch/completion handling
- **THEN** the system emits the same archive-complete semantics used by normal archive success
- **AND** merge handling returns `MergeDeferred`
- **AND** the change remains in `MergeWait`

#### Scenario: Mixed archiving restart does not drop archived change from queue lifecycle

- **GIVEN** three parallel workspaces are reused after an interrupted run
- **AND** two workspaces are still `WorkspaceState::Archiving`
- **AND** one workspace is already `WorkspaceState::Archived`
- **WHEN** the restarted parallel run resumes those workspaces
- **THEN** all three changes converge to archive-complete merge handling as their resume paths finish
- **AND** none of the resumed changes regresses to `NotQueued` solely because archive completed before shutdown

#### Scenario: Already merged archived worktree is terminal residue

- **GIVEN** a leftover parallel worktree contains an archived change entry for `alpha`
- **AND** workspace-local Git/base-branch comparison shows `alpha` is already merged into the base branch
- **WHEN** scheduler queue reconciliation scans existing worktrees
- **THEN** the worktree is not added to scheduler-local queued work as an archived-dirty repair candidate
- **AND** apply, acceptance, and archive are not run for `alpha`
- **AND** no user-visible archived-dirty repair diagnostic is emitted for `alpha`

#### Scenario: Non-merged archived dirty worktree remains repairable

- **GIVEN** a leftover parallel worktree contains an archived change entry for `beta`
- **AND** workspace-local Git/base-branch comparison does not show `beta` as merged
- **AND** the worktree represents an interrupted archive-finalization path
- **WHEN** scheduler queue reconciliation scans existing worktrees
- **THEN** `beta` may be added as an archived-dirty repair candidate
- **AND** the normal archive-complete merge handoff or repair path remains available
