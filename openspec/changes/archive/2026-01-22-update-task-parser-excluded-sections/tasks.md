## 1. Implementation
- [x] 1.1 `src/config/defaults.rs` の `ACCEPTANCE_SYSTEM_PROMPT` を更新し、Future Work / Out of Scope / Notes セクション内にチェックボックスが残っていたら FAIL とするチェックを追加する / 確認: 該当定数の文言を確認する
- [x] 1.2 `~/.config/opencode/command/cflx-apply.md` を更新し、Future Work / Out of Scope / Notes へ移動する際はチェックボックスを削除することを明確に指示する / 確認: 該当ファイルの文言を確認する

## 2. Validation
- [x] 2.1 `cargo test` を実行し、既存テストが通ることを確認する
- [x] 2.2 `cargo clippy` を実行し、警告がないことを確認する
