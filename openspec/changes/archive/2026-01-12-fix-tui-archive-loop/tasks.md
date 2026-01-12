# Tasks: TUIアーカイブループ修正

## 1. アーカイブパス検証の修正

- [x] 1.1 `src/tui/orchestrator.rs` の `archive_single_change()` 内のパス検証を修正
  - 現在: `openspec/archive`
  - 修正後: `openspec/changes/archive`

## 2. デバッグログの追加

- [x] 2.1 `archive_all_complete_changes()` の入り口にログを追加
  - 処理対象の変更数をログ出力
  - 各変更のアーカイブ開始/終了をログ出力

- [x] 2.2 `archive_single_change()` のパス検証結果をログ出力
  - 検証パスの実際の値を出力
  - exists() の結果を出力

## 3. テスト

- [x] 3.1 パス検証ロジックの単体テストを追加
- [x] 3.2 `cargo test` で既存テストがパスすることを確認
- [x] 3.3 `cargo clippy` でlintエラーがないことを確認

## 4. 検証

- [x] 4.1 TUIモードで複数の完了済み変更がある状態でアーカイブループが動作することを確認
  - E2Eテスト `test_archive_priority_multiple_complete_changes` でカバー済み
  - `cargo test` で全468テストがパス
  - `cargo clippy` でlintエラーなし
  - 手動TUIテストは実行環境による（`RUST_LOG=debug cargo run -- tui` で動作確認可能）
