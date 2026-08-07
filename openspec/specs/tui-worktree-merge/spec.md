# tui-worktree-merge Specification

## Purpose
TBD - created by archiving change add-worktree-view-with-merge. Update Purpose after archive.
## Requirements

### Requirement: Merge Error Handling

マージ失敗時 SHALL provide clear error messages and recovery guidance.

#### Scenario: コンフリクト発生時の自動abort

- **GIVEN** マージ中にコンフリクトが発生した
- **WHEN** `git merge` がコンフリクトを報告する
- **THEN** `git merge --abort` が自動実行される
- **AND** "Merge conflict detected. Merge aborted. Manual resolution required." エラーが表示される
- **AND** コンフリクトファイル一覧が含まれる

#### Scenario: エラーポップアップの表示

- **GIVEN** マージが失敗した
- **WHEN** エラーイベントが受信される
- **THEN** エラー詳細がポップアップで表示される
- **AND** ポップアップは任意のキーで閉じられる

#### Scenario: マージ失敗後の状態

- **GIVEN** マージが失敗した
- **WHEN** エラー処理が完了する
- **THEN** base repositoryの状態は変更されていない
- **AND** worktreeは削除されていない
- **AND** ユーザーは再試行または手動解決を選択できる

### Requirement: Merge Key Hint Display Conditions

TUI Worktree View SHALL display "M: merge" key hint only when ALL of the following conditions are met:
- Not main worktree
- Not detached HEAD
- No merge conflicts
- Has branch name
- Has commits ahead of base branch
- No resolve operation in progress

TUI SHALL NOT display merge key hint when resolve is in progress.

#### Scenario: M key hidden while resolve in progress
- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on a worktree that otherwise meets merge conditions
- **AND** a resolve operation is in progress
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

TUI SHALL detect whether an automatically inspectable worktree branch has commits ahead of the base branch during worktree list loading.

Detection SHALL run in parallel with conflict checking for eligible cache misses. Both periodic TUI and periodic Web/UDS refresh SHALL share the same observation cache. Ineligible worktrees and unchanged cache hits SHALL NOT spawn duplicate ahead/conflict commands. A skipped observation MUST NOT be represented as `has_commits_ahead = false` when that value would enable or suppress a merge action incorrectly.

Periodic filtering MUST NOT remove operator control. An operator-initiated merge or deletion SHALL perform a fresh targeted observation of the selected worktree before eligibility is decided, including branches such as `ws-session-*` that do not map to an OpenSpec change. A not-inspected periodic row SHALL receive an inspection-required diagnostic rather than the false message that it has no commits ahead.

<!-- Expected canonical result after archive: commits-ahead and conflict checks remain parallel for eligible cache misses but are not executed for stale/non-active worktrees or unchanged observations. -->

#### Scenario: Eligible active worktree is inspected

- **GIVEN** a secondary worktree maps to a current active or rejected change
- **AND** no matching cached observation exists
- **WHEN** the worktree list is loaded
- **THEN** commits-ahead detection and conflict checking run in parallel
- **AND** both complete before the checked observation is returned

#### Scenario: Ineligible worktree is fail-closed during periodic refresh

- **GIVEN** a secondary worktree does not map to a current active or rejected change
- **WHEN** either periodic refresh path loads the worktree list
- **THEN** commits-ahead and conflict commands are not executed for it
- **AND** merge eligibility does not infer clean or not-ahead status from the skipped checks
- **AND** the presentation reports that inspection is required rather than reporting no commits ahead

#### Scenario: Operator merge reinspects an unclassified worktree

- **GIVEN** a `ws-session-*` or other selected worktree was not inspected by periodic refresh
- **WHEN** the operator requests its merge
- **THEN** Conflux performs a fresh targeted ahead/conflict observation
- **AND** decides merge eligibility from that current repository evidence

#### Scenario: Operator deletion reinspects a stale worktree

- **GIVEN** a stale selected worktree was not inspected by periodic refresh
- **WHEN** the operator requests its deletion
- **THEN** Conflux performs a fresh targeted observation before deletion eligibility is decided
- **AND** periodic filtering alone does not make the worktree permanently undeletable

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
