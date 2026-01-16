# Tasks: リトライ処理を command-queue に統一

## タスク一覧

- [x] 1. `command_queue.rs` に `execute_with_retry_streaming()` メソッドを追加
  - streaming 出力対応版のリトライ実行メソッド
  - `Option<mpsc::Sender<OutputLine>>` で出力チャネルを受け取る
  - stderr を収集してリトライ判定に使用

- [x] 2. `command_queue.rs` に `stream_output()` ヘルパーメソッドを追加
  - stdout/stderr を並行して読み取り
  - 出力チャネルに送信しつつ stderr をバッファ

- [x] 3. `should_retry()` の判定条件を更新
  - 既存: パターンマッチ OR 短時間実行
  - 追加: エージェントクラッシュ（exit code != 0）

- [x] 4. `#[allow(dead_code)]` を `execute_with_retry()` 関連から削除
  - `is_retryable_error()`
  - `should_retry()`
  - `execute_with_retry()`

- [x] 5. `parallel/executor.rs` の `execute_apply_in_workspace()` を更新
  - 独自リトライロジック（534-562行目）を削除
  - iteration ループで自然にリトライされるよう変更

- [x] 6. `parallel/executor.rs` の `execute_archive_in_workspace()` を更新
  - 独自リトライロジック（997-1019行目）を削除
  - verification ループで自然にリトライされるよう変更

- [x] 7. `agent.rs` の `AgentRunner` を更新（必要に応じて）
  - `CommandQueue` への参照を適切に渡す（既に適切に使用済み）
  - 既存の streaming メソッドとの整合性確認（問題なし）

- [x] 8. 単体テストを追加
  - `execute_with_retry_streaming()` の正常系テスト
  - リトライ発生時のテスト
  - streaming 出力の検証

- [x] 9. Lint/Format チェック
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`

- [x] 10. 全テスト実行
  - `cargo test`
  - E2E テストでリトライ動作を確認
