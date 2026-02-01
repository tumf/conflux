## 1. Implementation
- [ ] 1.1 `src/tui/state/events/helpers.rs` の `AppState::update_changes` で、共有状態の `apply_count` を `iteration_number` に適用する際に単調増加マージ（既存値より小さい場合は維持）を実装する
  - 検証: `src/tui/state/events/helpers.rs` に `max` マージの分岐が追加されていることを確認する
- [ ] 1.2 自動更新でイテレーションが巻き戻らないことを検証するユニットテストを追加する
  - 検証: `src/tui/state/mod.rs` に新規テストを追加し、`cargo test tui_iteration_refresh_monotonic` が通る

## 2. Validation
- [ ] 2.1 `cargo test tui_iteration_refresh_monotonic` を実行し、テストが成功する
  - 検証: コマンドが終了コード0で完了する
