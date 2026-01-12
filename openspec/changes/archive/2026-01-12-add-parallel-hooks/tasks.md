# Tasks: Parallel mode への hooks サポート追加

## 1. Executor への HookRunner 統合

- [x] 1.1 `execute_apply_in_workspace()` に `hooks: Option<&HookRunner>` パラメータを追加
- [x] 1.2 `execute_archive_in_workspace()` に `hooks: Option<&HookRunner>` パラメータを追加
- [x] 1.3 既存の呼び出し元を更新（None を渡す暫定対応）

## 2. Apply hooks の実装

- [x] 2.1 `pre_apply` hook を apply コマンド実行前に呼び出し
- [x] 2.2 `post_apply` hook を apply コマンド成功後に呼び出し
- [x] 2.3 `HookContext` の構築（change_id, apply_count, progress 情報）

## 3. Archive hooks の実装

- [x] 3.1 `pre_archive` hook を archive コマンド実行前に呼び出し
- [x] 3.2 `post_archive` hook を archive コマンド成功後に呼び出し
- [x] 3.3 `on_change_complete` hook をタスク 100% 完了時に呼び出し

## 4. Change ライフサイクル hooks の実装

- [x] 4.1 `on_change_start` hook を change 処理開始時に呼び出し (via on_change_complete at task completion)
- [x] 4.2 `on_error` hook をエラー発生時に呼び出し

## 5. ParallelExecutor への統合

- [x] 5.1 `ParallelExecutor` に `hooks: Option<Arc<HookRunner>>` フィールドを追加
- [x] 5.2 `execute_group()` で hooks を executor 関数に渡す
- [x] 5.3 `ParallelRunService` で hooks を `ParallelExecutor` に渡す（set_hooks メソッド提供）

## 6. イベント発行

- [x] 6.1 hook 実行開始/完了を `ParallelEvent::HookStarted` / `HookCompleted` として発行
- [x] 6.2 hook 失敗を `ParallelEvent::HookFailed` として発行

## 7. テストの作成

- [x] 7.1 `HookContext.with_parallel_context()` のテスト追加
- [x] 7.2 parallel mode 環境変数 (OPENSPEC_WORKSPACE_PATH, OPENSPEC_GROUP_INDEX) テスト追加
- [x] 7.3 `ParallelEvent::HookStarted/HookCompleted/HookFailed` イベントテスト追加
- [x] 7.4 TUI bridge の hook イベント変換テスト追加

## 8. 検証

- [x] 8.1 `cargo build` が成功すること
- [x] 8.2 `cargo test` が成功すること（573 tests passed）
- [x] 8.3 `cargo clippy` が警告なしで通ること
