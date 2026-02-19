# Change: cflx server の data_dir 上書きオプションを明文化する

## Why
サーバデータの保存先は環境によって切り替えたいが、既定値と CLI 上書きの関係が仕様として明確ではないため、期待動作を明文化して利用者の混乱を避ける。

## What Changes
- `cflx server --data-dir <PATH>` によりサーバの永続データディレクトリを上書きできることを明文化する
- 未指定時は既定のデータディレクトリ（グローバル設定またはデフォルト）を使用することを明文化する

## Impact
- Affected specs: `specs/cli/spec.md`
- Affected code: `src/cli.rs`, `src/main.rs`, `src/config/mod.rs`, `src/config/defaults.rs`
