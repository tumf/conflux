# Spec: workspace-cleanup (Delta)

## MODIFIED Requirements

### Requirement: Workspace Cleanup Guard

`WorkspaceCleanupGuard`は、失敗したワークスペースを正しく保護し、正しい順序でクリーンアップを実行しなければならない（MUST）。

ガードは以下を実装する：
1. ワークスペース名とパスの両方を追跡する
2. `preserve()`で指定されたワークスペースはDrop時にクリーンアップしない
3. Drop時のクリーンアップでは、ワークツリー削除を先に実行し、その後ブランチを削除する

#### Scenario: 失敗したワークスペースの保護

- **GIVEN** ワークスペースAが作成され、トラッキングされている
- **AND** ワークスペースAの処理が失敗した
- **WHEN** `cleanup_guard.preserve("workspace-a")`が呼ばれる
- **AND** ガードがDrop時にクリーンアップを試みる
- **THEN** ワークスペースAはクリーンアップされない
- **AND** 他の保護されていないワークスペースはクリーンアップされる

#### Scenario: ワークツリー削除後のブランチ削除

- **GIVEN** ワークスペースがトラッキングされている（名前とパスの両方）
- **WHEN** ガードがDrop時にクリーンアップを実行する
- **THEN** 最初に`git worktree remove <path> --force`が実行される
- **AND** その後`git branch -D <branch_name>`が実行される
- **AND** Gitエラー（ブランチが使用中）が発生しない

#### Scenario: ワークスペースのトラッキング時にパスも保持

- **GIVEN** 新しいワークスペースが作成された（名前: "ws-test", パス: "/tmp/ws-test"）
- **WHEN** `cleanup_guard.track("ws-test", PathBuf::from("/tmp/ws-test"))`が呼ばれる
- **THEN** ガードはワークスペース名とパスの両方を保持する
- **AND** Drop時にパスを使用してワークツリーを削除できる

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
