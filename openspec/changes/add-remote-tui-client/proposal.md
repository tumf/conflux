# Change: リモートサーバ接続の TUI クライアントを追加する

## Why
サーバ常駐モードで複数プロジェクトを管理する場合、クライアント側はカレントディレクトリに依存せず、リモート状態を表示・操作できる必要がある。

## What Changes
- `--server <endpoint>` を追加し、TUI をリモートサーバに接続できるようにする
- プロジェクト単位のグルーピング表示とリモート状態の購読更新を追加する
- bearer token 認証のためのクライアント側トークン指定を追加する

## Impact
- Affected specs: cli, tui-architecture
- Affected code: `src/cli.rs`, `src/main.rs`, `src/tui/`, `src/remote/` (new)
