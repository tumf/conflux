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
