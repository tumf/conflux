# tui-worktree-merge 変更デルタ

## MODIFIED Requirements

### Requirement: Merge Key Hint Display Conditions

TUI Worktree View SHALL display "M: merge" key hint only when ALL of the following conditions are met:
- Not main worktree
- Not detached HEAD
- No merge conflicts
- Has branch name
- Has commits ahead of base branch

TUI SHALL NOT display merge key hint when worktree branch has no commits ahead of base branch.

#### Scenario: Mキーは差分がある場合のみ表示

- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on a worktree that is not main, not detached, has no conflicts, and has a branch name
- **AND** the worktree branch has commits ahead of base branch
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL include "M: merge"

#### Scenario: 差分がない場合はMキーを非表示

- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on a worktree that meets all conditions EXCEPT has no commits ahead
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL NOT include "M: merge"

#### Scenario: main worktreeではMキーを非表示

- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on main worktree
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL NOT include "M: merge"

### Requirement: Merge Request Error Messages

When merge request fails validation, TUI SHALL display clear warning message indicating the reason.

`request_merge_worktree_branch()` SHALL set appropriate warning message for each failure condition.

#### Scenario: view_mode条件の失敗メッセージ

- **GIVEN** M key is pressed
- **AND** view_mode is not Worktrees
- **WHEN** merge request validation runs
- **THEN** warning message SHALL be set to "Switch to Worktrees view to merge"
- **AND** merge request SHALL return None

#### Scenario: worktrees空の失敗メッセージ

- **GIVEN** M key is pressed in Worktrees view
- **AND** worktrees list is empty
- **WHEN** merge request validation runs
- **THEN** warning message SHALL be set to "No worktrees loaded"
- **AND** merge request SHALL return None

#### Scenario: カーソル範囲外の失敗メッセージ

- **GIVEN** M key is pressed in Worktrees view
- **AND** cursor index is out of bounds
- **WHEN** merge request validation runs
- **THEN** warning message SHALL contain cursor position and list length
- **AND** merge request SHALL return None

#### Scenario: 差分なしの失敗メッセージ

- **GIVEN** M key is pressed in Worktrees view
- **AND** selected worktree has no commits ahead of base
- **WHEN** merge request validation runs
- **THEN** warning message SHALL be "Cannot merge: no commits ahead of base branch"
- **AND** merge request SHALL return None

### Requirement: Worktree Commits Ahead Detection

TUI SHALL detect whether worktree branch has commits ahead of base branch during worktree list loading.

Detection SHALL run in parallel with conflict checking for performance.

#### Scenario: baseより先のコミットを検出

- **GIVEN** a worktree with branch that has 2 commits ahead of base
- **WHEN** worktree list is loaded with ahead detection
- **THEN** WorktreeInfo.has_commits_ahead SHALL be true

#### Scenario: 差分なしを検出

- **GIVEN** a worktree with branch at same commit as base
- **WHEN** worktree list is loaded with ahead detection
- **THEN** WorktreeInfo.has_commits_ahead SHALL be false

#### Scenario: 並列実行での差分チェック

- **GIVEN** multiple worktrees exist
- **WHEN** worktree list is loaded
- **THEN** commits ahead detection SHALL run in parallel using JoinSet
- **AND** conflict checking SHALL also run in parallel
- **AND** both checks SHALL complete before worktree list is returned

### Requirement: Merge Execution on Base Repository

Worktree branch merge SHALL be executed on base repository (main worktree), NOT on the worktree itself.

Working directory clean check SHALL be performed on base repository.

#### Scenario: base側でマージを実行

- **GIVEN** user presses M key on a mergeable worktree
- **WHEN** merge command is executed
- **THEN** `git merge` SHALL run in repo_root (base repository) directory
- **AND** `git merge` SHALL NOT run in worktree directory

#### Scenario: base側のworking directory cleanチェック

- **GIVEN** base repository has uncommitted changes
- **AND** worktree has uncommitted changes
- **WHEN** user attempts to merge the worktree branch
- **THEN** merge SHALL fail with "Working directory is not clean" error
- **AND** error message SHALL refer to base repository state

#### Scenario: worktree側のdirty状態はマージをブロックしない

- **GIVEN** base repository is clean (no uncommitted changes)
- **AND** worktree has uncommitted changes
- **WHEN** user attempts to merge the worktree branch
- **THEN** merge SHALL succeed
- **AND** worktree uncommitted changes SHALL remain intact
