# Change: run サブコマンドの追加

## Why

現在の CLI はサブコマンド構造を持っておらず、実行時に直接オーケストレーションループが開始される。
将来的に `status`、`config` などのサブコマンドを追加する計画があるため、CLI を拡張可能なサブコマンド構造に変更する必要がある。

## What Changes

- 現在の実行機能を `run` サブコマンドに移動
- clap の `#[command(subcommand)]` を使用したサブコマンド構造の導入
- 引数なしで実行した場合はヘルプを表示

## Impact

- Affected specs: cli (新規)
- Affected code: `src/cli.rs`, `src/main.rs`
- **BREAKING**: コマンドライン引数の構造が変更される
  - 旧: `openspec-orchestrator --change foo`
  - 新: `openspec-orchestrator run --change foo`
