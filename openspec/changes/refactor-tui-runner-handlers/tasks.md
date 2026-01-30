## 1. 実装
- [ ] 1.1 キーイベント処理をヘルパー関数へ抽出する（例: タブ切替／カーソル移動／エディタ起動／ワークツリー操作）
  - 検証: `src/tui/runner.rs` の `run_tui_loop` が新しいヘルパー関数を呼び出していることを確認する
- [ ] 1.2 TuiCommand の処理を専用ヘルパー関数に抽出する
  - 検証: `src/tui/runner.rs` の TuiCommand match がヘルパー関数経由になっていることを確認する
- [ ] 1.3 既存の動作が変わらないことを検証する
  - 検証: `cargo fmt && cargo clippy -- -D warnings && cargo test --bin cflx tui::runner::`
