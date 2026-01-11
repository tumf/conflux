# Change: エラー時のワークスペース保持

## Why

並列実行モードで最大イテレーション到達やその他のエラーが発生した場合、現在の実装ではworkspaceを自動的にクリーンアップ（削除）している。これにより：

1. **作業の喪失**: 途中まで進んでいたタスクの作業結果が失われる
2. **復旧の困難**: `add-workspace-resume` 機能があっても、workspaceが削除されているため復旧できない
3. **デバッグの困難**: 問題が発生した状態を調査できない

## What Changes

- エラー発生時はworkspaceを削除せず保持する
- エラーログにworkspace名を明示的に出力する
- `add-workspace-resume` と組み合わせることで、再実行時に自動的にresumeされる

### 詳細

1. **エラー時の保持**: `execute_apply_in_workspace` がエラーを返した場合、`cleanup_workspace` を呼び出さない
2. **ログ出力**: エラー時に `[ERROR] Failed for {change_id}, workspace preserved: {workspace_name}` を出力
3. **連携**: 次回実行時に `add-workspace-resume` の `find_existing_workspace` が保持されたworkspaceを検出し、自動的にresumeする

## Impact

- 影響するspec: `parallel-execution`
- 影響するコード:
  - `src/parallel/mod.rs` (エラーハンドリング変更)
  - `src/parallel/cleanup.rs` (クリーンアップロジック変更)
- 連携するchange: `add-workspace-resume`
