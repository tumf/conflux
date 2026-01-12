# Tasks: Parallel mode への hooks サポート追加

## 1. Executor への HookRunner 統合

- [ ] 1.1 `execute_apply_in_workspace()` に `hooks: Option<&HookRunner>` パラメータを追加
- [ ] 1.2 `execute_archive_in_workspace()` に `hooks: Option<&HookRunner>` パラメータを追加
- [ ] 1.3 既存の呼び出し元を更新（None を渡す暫定対応）

## 2. Apply hooks の実装

- [ ] 2.1 `pre_apply` hook を apply コマンド実行前に呼び出し
- [ ] 2.2 `post_apply` hook を apply コマンド成功後に呼び出し
- [ ] 2.3 `HookContext` の構築（change_id, apply_count, progress 情報）

## 3. Archive hooks の実装

- [ ] 3.1 `pre_archive` hook を archive コマンド実行前に呼び出し
- [ ] 3.2 `post_archive` hook を archive コマンド成功後に呼び出し
- [ ] 3.3 `on_change_complete` hook をタスク 100% 完了時に呼び出し

## 4. Change ライフサイクル hooks の実装

- [ ] 4.1 `on_change_start` hook を change 処理開始時に呼び出し
- [ ] 4.2 `on_error` hook をエラー発生時に呼び出し

## 5. ParallelExecutor への統合

- [ ] 5.1 `ParallelExecutor` に `hooks: Option<HookRunner>` フィールドを追加
- [ ] 5.2 `execute_group()` で hooks を executor 関数に渡す
- [ ] 5.3 `ParallelRunService` で hooks を `ParallelExecutor` に渡す

## 6. イベント発行

- [ ] 6.1 hook 実行開始/完了を `ParallelEvent` として発行
- [ ] 6.2 hook 失敗を `ParallelEvent::Error` として発行

## 7. テストの作成

- [ ] 7.1 parallel mode での `pre_apply` hook 実行テスト
- [ ] 7.2 parallel mode での `post_archive` hook 実行テスト
- [ ] 7.3 hook 失敗時の動作テスト（continue_on_failure = true/false）

## 8. 検証

- [ ] 8.1 `cargo build` が成功すること
- [ ] 8.2 `cargo test` が成功すること
- [ ] 8.3 `cargo clippy` が警告なしで通ること
- [ ] 8.4 parallel mode で hooks が実行されることを確認（手動テスト）
- [ ] 8.5 serial mode の hooks が引き続き動作することを確認（回帰テスト）
