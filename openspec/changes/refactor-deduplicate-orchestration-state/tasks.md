## 1. 事前検証（Characterization Tests）
- [x] 1.1 `Orchestrator::run()` の serial モードでの apply_counts・iteration 増加を検証する characterization test を追加
- [x] 1.2 `tui/orchestrator.rs` の `run_orchestrator()` で pending_changes が正しく減少することを検証する characterization test を追加
- [x] 1.3 `OrchestratorState` の `apply_count()`, `iteration()`, `pending_changes()`, `changes_processed()` の既存テストが全パス通過を確認
- [x] 1.4 全テスト（`cargo test`）が通ることを確認

## 2. OrchestratorState に不足フィールドを追加
- [x] 2.1 `stalled_change_ids: HashSet<String>` を `OrchestratorState` に追加し、対応する getter/setter を実装
- [x] 2.2 `skipped_change_ids: HashSet<String>` を `OrchestratorState` に追加し、対応する getter/setter を実装
- [x] 2.3 必要に応じて `error_histories` の管理メソッドを `OrchestratorState` に移動
- [x] 2.4 全テスト通過を確認

## 3. Orchestrator struct からローカル状態を削除
- [x] 3.1 `Orchestrator` struct から `apply_counts` フィールドを削除し、`shared_state.read().await.apply_count(id)` で参照するように変更
- [x] 3.2 `changes_processed` フィールドを削除し、`shared_state` の `changes_processed()` を使用
- [x] 3.3 `iteration` フィールドを削除し、`shared_state` の `iteration()` を使用
- [x] 3.4 `current_change_id` フィールドを削除し、`shared_state` の `current_change_id()` を使用
- [x] 3.5 `completed_change_ids`, `stalled_change_ids`, `skipped_change_ids` を `shared_state` に委譲
- [x] 3.6 全テスト通過を確認

## 4. TUI orchestrator からローカル状態を削除
- [x] 4.1 `run_orchestrator` 関数内の `apply_counts` ローカル変数を削除し、`shared_state` を参照
- [x] 4.2 `pending_changes` ローカル変数を削除し、`shared_state.pending_changes()` を参照
- [x] 4.3 `changes_processed`, `total_changes` を削除し、`shared_state` のカウンターを参照
- [x] 4.4 全テスト通過を確認、`cargo clippy -- -D warnings` クリア

## Acceptance #1 Failure Follow-up

- [x] `cargo fmt` のフォーマット変更（`src/orchestrator.rs`, `src/tui/orchestrator.rs`）をコミットし、git working tree をクリーンにする
