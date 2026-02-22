# 変更提案: cflx project 管理コマンドの追加

## Why（背景）
Conflux server のプロジェクト管理 API を CLI から直接操作できないため、TUI を使わない運用やスクリプト連携がしづらい。`cflx project` による add/remove/status/sync を追加し、サーバ管理を簡素化する。

## What Changes（変更内容）
- `cflx project add/remove/status/sync` を追加し、サーバのプロジェクト管理 API と git/sync を操作できるようにする。
- `--server` 未指定時はグローバル設定の `server.bind`/`server.port` を用いて接続先を解決する。
- 今回はサーバクライアント認証を扱わない（Authorization ヘッダは送らない、認証設定が必要な場合は明示的にエラー）。

## Impact（影響範囲）
- Affected specs: `openspec/specs/cli/spec.md`
- Affected code: `src/cli.rs`, `src/main.rs`, `src/remote/client.rs`, `src/remote/test_helpers.rs`
