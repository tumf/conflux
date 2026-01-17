## MODIFIED Requirements
### Requirement: Auto-Refresh Worktree List
Worktreeリスト SHALL be automatically refreshed without modifying tracked files in worktrees.

#### Scenario: 定期的な自動更新
- **GIVEN** Worktreeビューが表示されている
- **WHEN** 5秒経過する
- **THEN** worktreeリストが自動的に再取得される
- **AND** 衝突チェックは作業ツリーを変更しない
