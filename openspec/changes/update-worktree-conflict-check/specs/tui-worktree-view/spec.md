## MODIFIED Requirements
### Requirement: Auto-Refresh Worktree List
Worktreeリスト SHALL be automatically refreshed without modifying tracked files in worktrees.

衝突チェックは作業ツリーに影響を与えないGit手法で実行し、worktree上の作業状態を変更してはならない。

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
