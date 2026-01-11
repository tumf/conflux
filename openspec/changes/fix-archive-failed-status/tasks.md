# Tasks: アーカイブ失敗ステータス修正

## 1. 実装

- [ ] 1.1 `parallel_event_bridge.rs` の `ArchiveFailed` イベント処理を修正
  - `ProcessingError` イベントを追加で返すように変更
  - ログエントリと ProcessingError の両方を含む Vec を返す

## 2. テスト

- [ ] 2.1 `parallel_event_bridge.rs` の既存テストを確認
- [ ] 2.2 `ArchiveFailed` イベント変換のテストを追加

## 3. 検証

- [ ] 3.1 `cargo fmt --check` でフォーマット確認
- [ ] 3.2 `cargo clippy -- -D warnings` でリント確認
- [ ] 3.3 `cargo test` でテスト実行
