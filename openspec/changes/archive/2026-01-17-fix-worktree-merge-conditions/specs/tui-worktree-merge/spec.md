# tui-worktree-merge Change Delta

## ADDED Requirements

### Requirement: Merge Key Hint Display Conditions

TUI Worktree View SHALL display "M: merge" key hint only when ALL of the following conditions are met:
- Not main worktree
- Not detached HEAD
- No merge conflicts
- Has branch name
- Has commits ahead of base branch

TUI SHALL NOT display merge key hint when worktree branch has no commits ahead of base branch.

#### Scenario: M key displayed only when commits ahead

- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on a worktree that is not main, not detached, has no conflicts, and has a branch name
- **AND** the worktree branch has commits ahead of base branch
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL include "M: merge"

#### Scenario: M key hidden when no commits ahead

- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on a worktree that meets all conditions EXCEPT has no commits ahead
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL NOT include "M: merge"

#### Scenario: M key hidden for main worktree

- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on main worktree
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL NOT include "M: merge"

### Requirement: Merge Request Error Messages

When merge request fails validation, TUI SHALL display clear warning message indicating the reason.

`request_merge_worktree_branch()` SHALL set appropriate warning message for each failure condition.

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

### Requirement: Worktree Commits Ahead Detection

TUI SHALL detect whether worktree branch has commits ahead of base branch during worktree list loading.

Detection SHALL run in parallel with conflict checking for performance.

#### Scenario: Detect commits ahead of base

- **GIVEN** a worktree with branch that has 2 commits ahead of base
- **WHEN** worktree list is loaded with ahead detection
- **THEN** WorktreeInfo.has_commits_ahead SHALL be true

#### Scenario: Detect no commits ahead

- **GIVEN** a worktree with branch at same commit as base
- **WHEN** worktree list is loaded with ahead detection
- **THEN** WorktreeInfo.has_commits_ahead SHALL be false

#### Scenario: Parallel execution of commits ahead check

- **GIVEN** multiple worktrees exist
- **WHEN** worktree list is loaded
- **THEN** commits ahead detection SHALL run in parallel using JoinSet
- **AND** conflict checking SHALL also run in parallel
- **AND** both checks SHALL complete before worktree list is returned

### Requirement: Merge Execution on Base Repository

Worktree branch merge SHALL be executed on base repository (main worktree), NOT on the worktree itself.

Working directory clean check SHALL be performed on base repository.

#### Scenario: Execute merge on base side

- **GIVEN** user presses M key on a mergeable worktree
- **WHEN** merge command is executed
- **THEN** `git merge` SHALL run in repo_root (base repository) directory
- **AND** `git merge` SHALL NOT run in worktree directory

#### Scenario: Working directory clean check on base side

- **GIVEN** base repository has uncommitted changes
- **AND** worktree has uncommitted changes
- **WHEN** user attempts to merge the worktree branch
- **THEN** merge SHALL fail with "Working directory is not clean" error
- **AND** error message SHALL refer to base repository state

#### Scenario: Worktree dirty state does not block merge

- **GIVEN** base repository is clean (no uncommitted changes)
- **AND** worktree has uncommitted changes
- **WHEN** user attempts to merge the worktree branch
- **THEN** merge SHALL succeed
- **AND** worktree uncommitted changes SHALL remain intact

### Requirement: Merge Operation Debug Logging

TUI SHALL log debug information for merge operations to enable troubleshooting.

Merge operation SHOULD NOT crash TUI silently; errors SHALL be displayed to user.

#### Scenario: Debug log output when M key is pressed

- **GIVEN** RUST_LOG=debug is set
- **AND** user is in Worktrees view
- **WHEN** M key is pressed
- **THEN** debug log SHALL include view_mode value
- **AND** debug log SHALL include worktrees.len() value
- **AND** debug log SHALL include worktree_cursor_index value
- **AND** debug log SHALL include result of request_merge_worktree_branch()

#### Scenario: Debug log during merge command execution

- **GIVEN** RUST_LOG=debug is set
- **AND** merge command is being processed
- **WHEN** TuiCommand::MergeWorktreeBranch is received
- **THEN** debug log SHALL include worktree_path
- **AND** debug log SHALL include branch_name
- **AND** debug log SHALL include merge execution directory (repo_root)

#### Scenario: TUI stability on error

- **GIVEN** merge operation encounters an error
- **WHEN** error occurs during merge processing
- **THEN** TUI SHALL NOT crash silently
- **AND** error SHALL be displayed via warning_popup or log entry
- **AND** TUI SHALL remain operational
