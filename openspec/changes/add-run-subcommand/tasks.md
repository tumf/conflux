# Tasks: run サブコマンドの追加

## 1. CLI 構造の変更

- [ ] 1.1 `src/cli.rs` に `Commands` enum を追加（`Run` サブコマンドを含む）
- [ ] 1.2 `Run` サブコマンドに既存のオプション（`--change`, `--opencode-path`, `--openspec-cmd`）を移動
- [ ] 1.3 `Cli` 構造体を更新してサブコマンドを受け取るように変更

## 2. main.rs の更新

- [ ] 2.1 `main.rs` でサブコマンドに応じた処理分岐を追加
- [ ] 2.2 `run` サブコマンド実行時に既存の `Orchestrator::run()` を呼び出す

## 3. テストと検証

- [ ] 3.1 `cargo build` で正常にビルドできることを確認
- [ ] 3.2 `cargo clippy` でリントエラーがないことを確認
- [ ] 3.3 `openspec-orchestrator --help` でサブコマンド一覧が表示されることを確認
- [ ] 3.4 `openspec-orchestrator run --help` で run オプションが表示されることを確認
- [ ] 3.5 既存のテストが通ることを確認（`cargo test`）
