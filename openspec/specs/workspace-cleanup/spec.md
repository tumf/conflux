# workspace-cleanup Specification

## Purpose
Defines workspace cleanup behavior after parallel execution.
## Requirements
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

### Requirement: Guard Integration with Parallel Executor

`ParallelExecutor::execute_group()`メソッドは、失敗したワークスペースに対して`cleanup_guard.preserve()`を呼び出さなければならない（MUST）。

#### Scenario: 失敗したワークスペースの保護呼び出し

- **GIVEN** 並列実行で3つの変更を処理中
- **WHEN** 1つの変更が失敗する（`WorkspaceResult.error.is_some()`）
- **THEN** 失敗したワークスペースに対して`cleanup_guard.preserve(workspace_name)`が呼ばれる
- **AND** エラーログに「workspace preserved」が出力される
- **AND** `WorkspacePreserved`イベントが発行される

#### Scenario: 保護されたワークスペースは正常系クリーンアップでスキップ

- **GIVEN** 失敗したワークスペースAが`preserve()`で保護されている
- **AND** 成功したワークスペースBが存在する
- **WHEN** 正常系のクリーンアップループが実行される
- **THEN** ワークスペースAはスキップされる（`failed_workspace_names`に含まれるため）
- **AND** ワークスペースBは正常にクリーンアップされる
- **AND** `cleanup_guard.commit()`が呼ばれる

#### Scenario: 早期リターン時の保護されたワークスペースのスキップ

- **GIVEN** 失敗したワークスペースAが`preserve()`で保護されている
- **AND** ワークスペースBはトラッキングされているが保護されていない
- **WHEN** 関数が早期リターンし、`cleanup_guard`がDropされる
- **THEN** ワークスペースAはクリーンアップされない
- **AND** ワークスペースBはクリーンアップされる

### Requirement: Cleanup Logging

クリーンアップガードは、ワークツリーとブランチの両方の削除ログを出力しなければならない（MUST）。

#### Scenario: ワークツリー削除の成功ログ

- **WHEN** ガードがワークツリー削除に成功する
- **THEN** `"Successfully removed worktree '<name>'"`がdebugログに出力される

#### Scenario: ワークツリー削除の失敗ログ

- **WHEN** ワークツリー削除が失敗する
- **THEN** `"Failed to remove worktree '<name>': <error>"`がdebugログに出力される
- **AND** クリーンアップは続行される（次のワークスペースやブランチ削除）

#### Scenario: ブランチ削除の失敗ログ

- **WHEN** ブランチ削除が失敗する
- **THEN** `"Failed to delete git branch '<name>': <error>"`がdebugログに出力される
- **AND** エラーは抑制される（パニックしない）
