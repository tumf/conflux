## 1. 仕様更新
- [ ] 1.1 `openspec/changes/update-server-data-dir-option/specs/cli/spec.md` に `cflx server --data-dir` の要件とシナリオを追加する（確認: 追加した Requirement に `#### Scenario:` がある）

## 2. 実装
- [ ] 2.1 `src/cli.rs` の server サブコマンドヘルプに `--data-dir` の説明が含まれることを確認し、必要なら追記する（確認: `cflx server --help` の文面に `--data-dir` が出る）
- [ ] 2.2 `src/config/mod.rs` の `ServerConfig::apply_cli_overrides` が `data_dir` を上書きすることを確認するテストを追加する（確認: `cargo test` で新しいテストが通る）

## 3. 検証
- [ ] 3.1 `cargo test` を実行し、追加したテストが成功することを確認する
