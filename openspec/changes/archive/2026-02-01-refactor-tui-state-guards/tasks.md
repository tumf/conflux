## 1. 実装
- [x] 1.1 `request_merge_worktree_branch` のガード判定をヘルパーに抽出する
  - 検証: `src/tui/state/mod.rs` がヘルパー経由で判定していることを確認する
  - 完了: `src/tui/state/guards.rs` に `validate_view_mode`, `validate_not_resolving`, `validate_worktrees_not_empty`, `validate_cursor_in_bounds`, `validate_worktree_mergeable` が実装済み
- [x] 1.2 `toggle_selection` の分岐／ガードロジックをヘルパーに抽出する
  - 検証: 主要な分岐がヘルパー関数に移動していることを確認する
  - 完了: `src/tui/state/guards.rs` に `validate_change_toggleable`, `handle_toggle_select_mode`, `handle_toggle_running_mode`, `handle_toggle_stopped_mode` が実装済み
- [x] 1.3 リファクタリング後の挙動が維持されることを検証する
  - 検証: `cargo fmt && cargo clippy -- -D warnings && cargo test --bin cflx tui::state::`
  - 完了: すべての検証コマンドが成功（118テスト全てパス）
