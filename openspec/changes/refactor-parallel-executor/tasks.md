## 1. 準備

- [ ] 1.1 `src/parallel/` ディレクトリを作成
- [ ] 1.2 共通型を `src/parallel/types.rs` に移動（WorkspaceResult）

## 2. イベント関連の分離

- [ ] 2.1 `ParallelEvent` enum を `src/parallel/events.rs` に移動
- [ ] 2.2 `send_event` ヘルパーメソッドを events モジュールに移動

## 3. クリーンアップガードの分離

- [ ] 3.1 `WorkspaceCleanupGuard` を `src/parallel/cleanup.rs` に移動
- [ ] 3.2 Drop 実装を含めて移動

## 4. コンフリクト処理の分離

- [ ] 4.1 `detect_conflicts` を `src/parallel/conflict.rs` に移動
- [ ] 4.2 `resolve_conflicts_with_retry` を移動
- [ ] 4.3 関連するヘルパー関数を移動

## 5. 実行ロジックの分離

- [ ] 5.1 `execute_apply_in_workspace` を `src/parallel/executor.rs` に移動
- [ ] 5.2 `execute_archive_in_workspace` を移動
- [ ] 5.3 `check_task_progress` を移動

## 6. オーケストレーション層の整理

- [ ] 6.1 残りの `ParallelExecutor` メソッドを `src/parallel/mod.rs` に配置
- [ ] 6.2 `src/parallel_executor.rs` を削除
- [ ] 6.3 `src/parallel/mod.rs` から必要な型を re-export

## 7. 依存関係の更新

- [ ] 7.1 `parallel_run_service.rs` のインポートを更新
- [ ] 7.2 `tui/parallel_event_bridge.rs` のインポートを更新
- [ ] 7.3 その他の参照箇所を更新

## 8. テストと検証

- [ ] 8.1 既存テストを適切なモジュールに移動
- [ ] 8.2 `cargo test` で全テスト通過を確認
- [ ] 8.3 `cargo clippy` で警告がないことを確認
