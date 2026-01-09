# Change: 環境変数で openspec コマンドを設定可能にする

## Why

現在、`openspec` コマンドのパスは CLI 引数 `--openspec-cmd` でのみ設定できる。
CI/CD 環境やコンテナ環境では、環境変数で設定できる方が便利なケースが多い。
特に、設定をシェルプロファイルや `.env` ファイルで一元管理したい場合に有用。

## What Changes

- CLI 引数 `--openspec-cmd` に加えて、環境変数 `OPENSPEC_CMD` でも設定できるようにする
- 優先順位: CLI 引数 > 環境変数 > デフォルト値
- clap の `env` 属性を使用してシンプルに実装

## Impact

- Affected specs: configuration
- Affected code: `src/cli.rs`
- Breaking changes: なし（後方互換性あり）
