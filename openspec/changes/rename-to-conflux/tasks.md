# Implementation Tasks

## 1. Rustパッケージとバイナリのリネーム

- [ ] 1.1 `Cargo.toml` の `[package].name` を `conflux` に変更
- [ ] 1.2 `Cargo.toml` に `[[bin]]` セクションを追加して `name = "cflx"` を指定
- [ ] 1.3 `src/cli.rs` の `#[command(name = "...")]` を `cflx` に変更
- [ ] 1.4 `src/cli.rs` のすべてのテストケースで `openspec-orchestrator` → `cflx` に更新

## 2. 設定ファイルパスの更新

- [ ] 2.1 `src/config/defaults.rs` の `PROJECT_CONFIG_FILE` を `.cflx.jsonc` に変更
- [ ] 2.2 `src/config/defaults.rs` の `GLOBAL_CONFIG_DIR` を `cflx` に変更
- [ ] 2.3 `src/config/mod.rs` のドキュメントコメントを更新
- [ ] 2.4 `src/main.rs` の `init` コマンドで生成するパスを `.cflx.jsonc` に変更
- [ ] 2.5 旧設定ファイル名の探索ロジックを削除（破壊的変更として）

## 3. ユーザー向けメッセージの更新

- [ ] 3.1 `src/orchestrator.rs` の承認案内メッセージを `cflx approve set` に変更
- [ ] 3.2 `src/templates.rs` のテンプレートコメントを更新
- [ ] 3.3 エラーメッセージ内の `openspec-orchestrator` 参照を `cflx` に更新

## 4. ドキュメントの更新

- [ ] 4.1 `README.md` の実行例をすべて `cflx` に更新
- [ ] 4.2 `README.md` の設定ファイルパスを `.cflx.jsonc` に更新
- [ ] 4.3 `README.ja.md` の実行例をすべて `cflx` に更新
- [ ] 4.4 `README.ja.md` の設定ファイルパスを `.cflx.jsonc` に更新
- [ ] 4.5 `DEVELOPMENT.md` の設定ファイルパス参照を更新
- [ ] 4.6 `AGENTS.md` の設定ファイルパス参照を更新
- [ ] 4.7 `openspec/project.md` のプロダクト説明を更新

## 5. テストの更新と検証

- [ ] 5.1 すべてのユニットテストで CLI名を `cflx` に更新
- [ ] 5.2 `cargo fmt` 実行
- [ ] 5.3 `cargo clippy -- -D warnings` 実行
- [ ] 5.4 `cargo test` 実行してすべてのテストがパス
- [ ] 5.5 `cargo build --release` で実行ファイル名が `cflx` であることを確認

## 6. 設定ファイルサンプルの更新

- [ ] 6.1 `.openspec-orchestrator.claude.jsonc` を `.cflx.claude.jsonc` にリネーム
- [ ] 6.2 `.openspec-orchestrator.opencode.jsonc` を `.cflx.opencode.jsonc` にリネーム
- [ ] 6.3 各サンプルファイル内のコメントを更新
- [ ] 6.4 `.gitignore` / `.git/info/exclude` のパターンを更新

## 7. 最終確認

- [ ] 7.1 `cflx --version` でバージョン表示を確認
- [ ] 7.2 `cflx init` で `.cflx.jsonc` が生成されることを確認
- [ ] 7.3 `cflx run --help` でヘルプメッセージを確認
- [ ] 7.4 既存のワークフローが動作することを手動テスト
