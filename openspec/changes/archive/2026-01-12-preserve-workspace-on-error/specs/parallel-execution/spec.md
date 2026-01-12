# Spec: parallel-execution (Delta)

## ADDED Requirements

### Requirement: Workspace Preservation on Error

並列実行においてエラーが発生した場合、workspaceを削除せずに保持しなければならない（MUST）。

#### Scenario: Workspace preserved on max iterations

- Given: applyループが最大イテレーション数に到達した
- When: エラーが発生する
- Then: workspaceは削除されず保持される
- And: `[ERROR] Failed for {change_id}, workspace preserved: {workspace_name}` がログ出力される

#### Scenario: Workspace preserved on apply failure

- Given: applyコマンドが非ゼロ終了コードで失敗した
- When: エラーが発生する
- Then: workspaceは削除されず保持される
- And: エラーログにworkspace名が含まれる

#### Scenario: Workspace cleaned up on success

- Given: changeの処理が正常に完了した（apply + archive）
- When: 処理完了後
- Then: workspaceは通常通りクリーンアップされる

#### Scenario: Resume hint logged on error

- Given: workspaceがエラーにより保持された
- When: エラーログが出力される
- Then: `[INFO] To resume: run with the same change_id, workspace will be automatically detected` が出力される

### Requirement: WorkspacePreserved Event

エラー時にworkspaceが保持された場合、TUIに通知するイベントを発行しなければならない（MUST）。

#### Scenario: WorkspacePreserved event emitted

- Given: エラーによりworkspaceが保持された
- When: クリーンアップフェーズがスキップされる
- Then: `ParallelEvent::WorkspacePreserved { change_id, workspace_name }` イベントが発行される

#### Scenario: TUI displays preserved workspace

- Given: TUIモードで実行中
- When: `WorkspacePreserved` イベントを受信
- Then: ログペインに「Workspace preserved: {workspace_name}」が表示される
