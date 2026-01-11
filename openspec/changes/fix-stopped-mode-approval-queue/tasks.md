# Tasks

## 1. Implementation

- [ ] 1.1 `src/tui/state/mod.rs` の `toggle_approval()` 関数を修正
  - `AppMode::Select | AppMode::Stopped` を `AppMode::Select` のみに変更
  - `AppMode::Running | AppMode::Stopped` で停止モードを Running と同様に扱う

## 2. Testing

- [ ] 2.1 ユニットテストを追加: 停止モードで承認時に `ApproveOnly` コマンドが返されることを確認
- [ ] 2.2 既存テストが通ることを確認 (`cargo test`)

## 3. Verification

- [ ] 3.1 `cargo fmt --check` でフォーマット確認
- [ ] 3.2 `cargo clippy` でリント確認
- [ ] 3.3 手動テスト: TUIで停止後に `@` キーで承認し、`[@]` 表示になることを確認
