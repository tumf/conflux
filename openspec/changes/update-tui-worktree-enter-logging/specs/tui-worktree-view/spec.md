## MODIFIED Requirements
### Requirement: WorktreeビューのEnter操作ガイダンス
WorktreesビューでEnterキーが無視される場合、TUIはその理由をwarningログとして表示しなければならない（SHALL）。

#### Scenario: Worktreesビュー以外でEnterが無視される場合の警告
- **GIVEN** TUIがWorktreesビュー以外を表示している
- **WHEN** ユーザーがEnterキーを押す
- **THEN** TUIは"Enter ignored: not in Worktrees view"をwarningログに出力する

#### Scenario: Worktreeが未選択のためEnterが無視される場合の警告
- **GIVEN** TUIがWorktreesビューを表示している
- **AND** 選択中のworktreeが存在しない
- **WHEN** ユーザーがEnterキーを押す
- **THEN** TUIは"Enter ignored: no worktree selected"をwarningログに出力する

#### Scenario: worktree_command未設定でEnterが無視される場合の警告
- **GIVEN** TUIがWorktreesビューを表示している
- **AND** worktree_commandが設定されていない
- **WHEN** ユーザーがEnterキーを押す
- **THEN** TUIは"Enter ignored: worktree_command not configured"をwarningログに出力する
