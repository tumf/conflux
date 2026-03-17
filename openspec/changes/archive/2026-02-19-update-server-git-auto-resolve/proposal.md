# Change: サーバの git/pull・git/push に auto_resolve オプションを追加する

## Why
server API のテストで、git/pull・git/push の非 fast-forward 時に resolve_command が実行されることを確認できる必要があるため。

## What Changes
- `POST /api/v1/projects/{id}/git/pull` と `POST /api/v1/projects/{id}/git/push` に `auto_resolve`（および `resolve_strategy`）を追加する
- `auto_resolve=true` のとき、非 fast-forward を検知したら resolve_command を実行し、解決後に処理を継続する
- `auto_resolve` 未指定の場合は従来どおり明示的エラーを返す

## Impact
- Affected specs: `openspec/specs/server-mode/spec.md`
- Affected code: `src/server/api.rs`, `src/server/registry.rs`, `src/config/mod.rs`, `src/agent/runner.rs`
