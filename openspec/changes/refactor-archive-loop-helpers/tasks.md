## 1. 実装
- [ ] 1.1 archive ループのフック実行・コマンド実行・検証・履歴記録をヘルパー関数に分割する
  - 検証: `src/execution/archive.rs` の `execute_archive_loop` がヘルパー関数を呼び出していることを確認する
- [ ] 1.2 リトライ回数や履歴伝播が維持されていることを確認する
  - 検証: `archive_change_streaming` の履歴が次回プロンプトに渡されるコード経路が残っていることを確認する
- [ ] 1.3 リファクタリング後の挙動が維持されることを検証する
  - 検証: `cargo fmt && cargo clippy -- -D warnings && cargo test --bin cflx execution::archive::`
