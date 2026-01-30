## 1. 実装
- [x] 1.1 `request_merge_worktree_branch` のガード判定をヘルパーに抽出する
  - 検証: `src/tui/state/mod.rs` がヘルパー経由で判定していることを確認する
- [x] 1.2 `toggle_selection` の分岐／ガードロジックをヘルパーに抽出する
  - 検証: 主要な分岐がヘルパー関数に移動していることを確認する
- [x] 1.3 リファクタリング後の挙動が維持されることを検証する
  - 検証: `cargo fmt && cargo clippy -- -D warnings && cargo test --bin cflx tui::state::`
