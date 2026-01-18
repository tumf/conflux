## MODIFIED Requirements
### Requirement: Merge Key Hint Display Conditions

TUI Worktree View SHALL display "M: merge" key hint only when ALL of the following conditions are met:
- Not main worktree
- Not detached HEAD
- No merge conflicts
- Has branch name
- Has commits ahead of base branch OR commits-ahead status is unknown
- No resolve operation in progress

TUI SHALL NOT display merge key hint when resolve is in progress.

When commits-ahead status is unknown, the key hint SHALL indicate a warning state so the user understands the merge will proceed with a warning.

#### Scenario: M key hidden while resolve in progress
- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on a worktree that otherwise meets merge conditions
- **AND** a resolve operation is in progress
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL NOT include "M: merge"

#### Scenario: M key shown with warning when commits-ahead is unknown
- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on a worktree with unknown commits-ahead status
- **AND** worktree otherwise meets merge conditions
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL include "M: merge"
- **AND** the merge key hint SHALL display a warning indicator

### Requirement: Merge Request Error Messages

When merge request fails validation, TUI SHALL display clear warning message indicating the reason.

`request_merge_worktree_branch()` SHALL set appropriate warning message for each failure condition.

When commits-ahead status is unknown, merge validation SHALL permit the request and set a warning message indicating the status could not be confirmed. Unknown status MUST NOT be treated as merged or no-ahead, and the M key MUST remain visible with a warning indicator.

#### Scenario: Failure message for view_mode condition

- **GIVEN** M key is pressed
- **AND** view_mode is not Worktrees
- **WHEN** merge request validation runs
- **THEN** warning message SHALL be set to "Switch to Worktrees view to merge"
- **AND** merge request SHALL return None

#### Scenario: Failure message for empty worktrees

- **GIVEN** M key is pressed in Worktrees view
- **AND** worktrees list is empty
- **WHEN** merge request validation runs
- **THEN** warning message SHALL be set to "No worktrees loaded"
- **AND** merge request SHALL return None

#### Scenario: Failure message for cursor out of range

- **GIVEN** M key is pressed in Worktrees view
- **AND** cursor index is out of bounds
- **WHEN** merge request validation runs
- **THEN** warning message SHALL contain cursor position and list length
- **AND** merge request SHALL return None

#### Scenario: Failure message for no commits ahead

- **GIVEN** M key is pressed in Worktrees view
- **AND** selected worktree has no commits ahead of base
- **WHEN** merge request validation runs
- **THEN** warning message SHALL be "Cannot merge: no commits ahead of base branch"
- **AND** merge request SHALL return None

#### Scenario: Warning message for unknown commits ahead

- **GIVEN** M key is pressed in Worktrees view
- **AND** selected worktree has unknown commits-ahead status
- **WHEN** merge request validation runs
- **THEN** warning message SHALL indicate commits-ahead status could not be confirmed
- **AND** merge request SHALL return Some merge request

### Requirement: Worktree Commits Ahead Detection

TUI SHALL detect whether worktree branch has commits ahead of base branch during worktree list loading.

Detection SHALL run in parallel with conflict checking for performance.

When detection fails, WorktreeInfo SHALL record commits-ahead status as unknown rather than false.

#### Scenario: Detect commits ahead of base

- **GIVEN** a worktree with branch that has 2 commits ahead of base
- **WHEN** worktree list is loaded with ahead detection
- **THEN** WorktreeInfo.has_commits_ahead SHALL be true

#### Scenario: Detect no commits ahead

- **GIVEN** a worktree with branch at same commit as base
- **WHEN** worktree list is loaded with ahead detection
- **THEN** WorktreeInfo.has_commits_ahead SHALL be false

#### Scenario: Record unknown commits-ahead on failure

- **GIVEN** commits-ahead detection fails for a worktree
- **WHEN** worktree list is loaded with ahead detection
- **THEN** WorktreeInfo.has_commits_ahead SHALL be unknown
- **AND** the failure SHALL NOT be treated as no commits ahead

#### Scenario: Parallel execution of commits ahead check

- **GIVEN** multiple worktrees exist
- **WHEN** worktree list is loaded
- **THEN** commits ahead detection SHALL run in parallel using JoinSet
- **AND** conflict checking SHALL also run in parallel
- **AND** both checks SHALL complete before worktree list is returned
