## 1. Implementation
- [ ] 1.1 `is_active` の change では Space(@/x) を無効化し、状態が変わらないことを保証する。
  - **Verify**: `src/tui/state/mod.rs` の `toggle_selection()` で `queue_status.is_active()` を判定し、変更なしで `None` を返すことを確認する。
- [ ] 1.2 `is_active` の change では @ を無効化し、承認状態が変わらないことを保証する。
  - **Verify**: `src/tui/state/mod.rs` の `toggle_approval()` で `queue_status.is_active()` を判定し、変更なしで `None` を返すことを確認する。
- [ ] 1.3 実行中の操作拒否を示す警告メッセージを設定する。
  - **Verify**: `warning_message` が設定されることをユニットテストで確認する。
- [ ] 1.4 ユニットテストを追加/更新する。
  - **Verify**: `cargo test tui::state` を実行し、`is_active` 状態で Space/@ が無効化されるテストが通ることを確認する。
