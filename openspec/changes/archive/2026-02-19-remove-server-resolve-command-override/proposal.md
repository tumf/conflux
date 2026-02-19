# Change: server.resolve_command を廃止し、共通 resolve_command を使用する

## Why
サーバの auto_resolve が別キー（server.resolve_command）を要求すると設定が二重化され、ユーザーが意図した resolve_command が使われないため。

## What Changes
- `server.resolve_command` を廃止し、サーバの auto_resolve はトップレベルの `resolve_command` を使用する
- `cflx server` の `--resolve-command` 受け付けを廃止する
- `server.resolve_command` が設定ファイルに存在する場合は設定エラーとして扱う

## Impact
- Affected specs: `openspec/specs/configuration/spec.md`, `openspec/specs/server-mode/spec.md`, `openspec/specs/cli/spec.md`
- Affected code: `src/config/mod.rs`, `src/cli.rs`, `src/server/mod.rs`, `src/server/api.rs`
