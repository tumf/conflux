# tui-worktree-view Specification

## Purpose
TBD - created by archiving change add-worktree-view-with-merge. Update Purpose after archive.
## Requirements

### Requirement: Auto-Refresh Worktree List
Worktreeリスト SHALL be automatically refreshed without modifying tracked files in worktrees.

衝突チェックは作業ツリーに影響を与えないGit手法で実行し、worktree上の作業状態を変更してはならない。

衝突チェックで `git merge-tree` を利用する場合、正しい引数形式で実行し、競合時はエラー扱いではなく競合ありとして判定しなければならない（MUST）。

#### Scenario: 定期的な自動更新
- **GIVEN** Worktreeビューが表示されている
- **WHEN** 5秒経過する
- **THEN** worktreeリストが自動的に再取得される
- **AND** 衝突チェックは作業ツリーを変更しない

#### Scenario: 衝突チェックは作業ツリーを変更しない
- **GIVEN** worktree上でエージェント作業が進行中である
- **WHEN** 5秒ごとの衝突チェックが実行される
- **THEN** worktree内の作業ツリーやインデックスは変更されない
- **AND** 進行中の作業は中断されない

#### Scenario: merge-tree 競合はエラー扱いにしない
- **GIVEN** worktreeブランチとベースブランチの間に競合が存在する
- **WHEN** 競合チェックが `git merge-tree --write-tree` で実行される
- **THEN** 競合は「競合あり」として判定される
- **AND** コマンド失敗として扱われない

### Requirement: Enter Key Operation Guidance

The TUI MUST display warning logs when the Enter key is ignored in Worktrees view, explaining the reason for rejection.

#### Scenario: Warning When Enter Is Ignored Outside Worktrees View

- **GIVEN** the TUI is displaying a view other than Worktrees
- **WHEN** the user presses the Enter key
- **THEN** the TUI outputs "Enter ignored: not in Worktrees view" to the warning log

#### Scenario: Warning When Enter Is Ignored Due to No Worktree Selection

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** no worktree is currently selected
- **WHEN** the user presses the Enter key
- **THEN** the TUI outputs "Enter ignored: no worktree selected" to the warning log

#### Scenario: Warning When Enter Is Ignored Due to Missing worktree_command Configuration

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** worktree_command is not configured
- **WHEN** the user presses the Enter key
- **THEN** the TUI outputs "Enter ignored: worktree_command not configured" to the warning log

### Requirement: Worktree deletion progress is visible in TUI

When a user confirms manual worktree deletion from the TUI Worktrees view, the TUI MUST immediately show that deletion is in progress for the target worktree until the deletion command completes.

The progress state MUST be transient UI state only. It MUST NOT be persisted or used as an input to scheduler dispatch, resume routing, acceptance, archive, or next-action decisions.

#### Scenario: Normal delete shows deleting row immediately

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** a non-main worktree is selected
- **WHEN** the user confirms deletion with `Y`
- **THEN** the selected worktree row shows `[Deleting...]` before the deletion command completes
- **AND** the TUI logs that worktree deletion has started

#### Scenario: Skip-teardown delete shows deleting row immediately

- **GIVEN** the TUI is displaying the worktree deletion confirmation modal
- **AND** the selected worktree has a teardown script that may be skipped
- **WHEN** the user confirms skip-teardown deletion with `S`
- **THEN** the selected worktree row shows `[Deleting...]` before the deletion command completes
- **AND** the TUI logs that deletion started with skip-teardown

#### Scenario: Deleting worktree suppresses duplicate target actions

- **GIVEN** a worktree row is marked as deleting in the TUI
- **WHEN** the user attempts to delete, merge, open, or edit that same worktree row
- **THEN** the TUI does not emit the normal target action command
- **AND** the TUI displays or logs a warning that the worktree is already being deleted

#### Scenario: Successful delete clears progress state

- **GIVEN** a worktree row is marked as deleting in the TUI
- **WHEN** the worktree deletion command succeeds
- **THEN** the deleting marker is cleared
- **AND** the existing worktree refresh behavior updates the Worktrees list
- **AND** existing worktree and branch deletion success or warning logs remain available

#### Scenario: Failed delete clears progress state and allows retry

- **GIVEN** a worktree row is marked as deleting in the TUI
- **WHEN** the worktree deletion command fails
- **THEN** the deleting marker is cleared
- **AND** the existing failure popup or error log is shown
- **AND** the worktree can be selected for deletion again

#### Scenario: Deletion progress remains UI-only

- **GIVEN** a worktree row is marked as deleting in the TUI
- **WHEN** the scheduler, resume routing, acceptance, archive, or next-action selection logic runs
- **THEN** those workflow decisions do not read or depend on the TUI deletion progress marker

### Requirement: Worktree command launch uses validated cwd

When the user creates a worktree from the TUI Worktrees view and Conflux is about to launch `worktree_command`, the system MUST verify that the target cwd is a materialized and registered Git worktree. The validation MUST use repository evidence: filesystem existence, directory type, usable Git worktree metadata or toplevel resolution, and registration in `git worktree list` for the base repository.

#### Scenario: valid worktree command launch

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** `worktree_command` is configured
- **AND** `git worktree add` creates a registered worktree with materialized files and Git metadata
- **AND** `.wt/setup` is absent or succeeds
- **WHEN** the user presses `+`
- **THEN** Conflux validates the created worktree path before launching the configured command
- **AND** the configured command is launched with the validated worktree path as its cwd

#### Scenario: invalid created worktree does not launch command

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** `worktree_command` is configured
- **WHEN** the user presses `+`
- **AND** the created worktree path is missing, not a directory, not a Git worktree, or not registered in `git worktree list`
- **THEN** Conflux does not launch `worktree_command`
- **AND** the TUI logs a diagnostic that identifies the invalid worktree path and validation failure

#### Scenario: deleted cwd before launch is suppressed

- **GIVEN** `git worktree add` initially creates a valid worktree for the TUI `+` action
- **AND** `.wt/setup` succeeds or is absent
- **WHEN** the worktree path is deleted or invalidated before the command-launch boundary
- **THEN** Conflux revalidates the path before command launch
- **AND** Conflux does not launch `worktree_command` in the deleted or invalid cwd
- **AND** the TUI logs that command launch was suppressed because the cwd is invalid

### Requirement: Setup failure cleanup is visible

When `.wt/setup` fails during TUI Worktrees `+` creation and Conflux removes the newly created worktree, the TUI MUST log the setup failure, the cleanup action, and the cleanup result with the affected path before returning control to the operator.

#### Scenario: setup failure cleanup logs path and result

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** `worktree_command` is configured
- **AND** `git worktree add` creates a valid worktree
- **WHEN** `.wt/setup` fails
- **THEN** Conflux logs the setup failure
- **AND** Conflux logs that it is cleaning up the created worktree path
- **AND** Conflux logs whether cleanup succeeded or failed
- **AND** Conflux does not launch `worktree_command`

### Requirement: Worktree command validation remains observability-only

TUI Worktrees `+` validation, diagnostics, and cleanup logging MUST NOT introduce durable workflow-control state outside workspace file state, workspace git state, or base-branch tree comparison.

#### Scenario: validation state is not workflow control input

- **GIVEN** TUI Worktrees `+` validation records diagnostics for an invalid or deleted command cwd
- **WHEN** scheduler dispatch, resume routing, acceptance gating, archive routing, or next-action selection runs
- **THEN** those workflow decisions do not depend on the TUI validation diagnostics or transient UI state
- **AND** deleting external logs or UI caches does not change the next action chosen for the same workspace contents
