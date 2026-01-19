## MODIFIED Requirements

### Requirement: Workspace Cleanup Guard

`WorkspaceCleanupGuard`は、成功時の明示的なクリーンアップのみを許可し、それ以外の経路ではworkspaceを保持しなければならない（MUST）。

ガードは以下を実装する：
1. ワークスペース名とパスの両方を追跡する
2. 成功時に明示的にcleanupが指示された場合のみworktreeとブランチを削除する
3. それ以外のDropや早期終了ではworkspaceを削除しない

#### Scenario: キャンセル時はDropでcleanupしない

- **GIVEN** ワークスペースが作成され、トラッキングされている
- **AND** 実行がキャンセルされる
- **WHEN** `WorkspaceCleanupGuard` がDropされる
- **THEN** worktreeは削除されない
- **AND** 再開可能な状態が維持される
