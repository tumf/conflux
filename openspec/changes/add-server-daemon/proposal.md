# Change: マルチプロジェクト用のサーバ常駐モードを追加する

## Why
カレントディレクトリ前提の実行では、複数プロジェクトを同時に運用・観測・制御することが難しい。サーバ常駐でプロジェクトを保持し、API 経由で一元操作できるようにすることで運用効率と安全性を高める。

## What Changes
- `cflx server` サブコマンドを追加し、グローバル設定のみで常駐起動する
- `remote_url + branch` をキーにしたプロジェクトレジストリと永続化ストアを追加する
- API v1 でプロジェクトの追加/削除/同期/実行制御を提供する
- ループバック以外の bind では bearer token 認証を必須化する
- `~/.wt/setup` は一切参照/実行しない

## Impact
- Affected specs: server-mode (new), cli, configuration
- Affected code: `src/cli.rs`, `src/main.rs`, `src/config/`, `src/server/` (new), `src/web/`
