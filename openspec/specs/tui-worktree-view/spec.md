# tui-worktree-view Specification

## Purpose
TBD - created by archiving change add-worktree-view-with-merge. Update Purpose after archive.
## Requirements
### Requirement: Auto-Refresh Worktree List
Worktreeリスト SHALL be automatically refreshed.

#### Scenario: 定期的な自動更新
- **GIVEN** Worktreeビューが表示されている
- **WHEN** 5秒経過する
- **THEN** worktreeリストが自動的に再取得される
- **AND** 表示が更新される

#### Scenario: Worktree作成後の即時更新
- **GIVEN** Worktreeビューでworktreeを作成した
- **WHEN** worktree_commandが完了する
- **THEN** worktreeリストが即座に更新される
- **AND** 新しいworktreeが表示される

#### Scenario: Worktree削除後の即時更新
- **GIVEN** Worktreeビューでworktreeを削除した
- **WHEN** 削除が完了する
- **THEN** worktreeリストが即座に更新される
- **AND** 削除されたworktreeが表示から消える

#### Scenario: デフォルトworktreeディレクトリの解決
- **GIVEN** `workspace_base_dir` が未設定
- **AND** TUI が worktree の作成先を決定する
- **THEN** デフォルトディレクトリは設定仕様に従って解決される
- **AND** worktree は `<data_dir>/conflux/worktrees/<project_slug>` 配下に作成される

