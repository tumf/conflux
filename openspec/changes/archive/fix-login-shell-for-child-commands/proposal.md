# Change: ユーザー設定コマンドをログインシェル経由で実行する

**Change Type**: implementation

## Why

`cflx server` が launchd 等の非ログインシェル環境から起動された場合、PATH が `/usr/bin:/bin:/usr/sbin:/sbin` のみに制限される。`resolve_command` や hooks, web API の worktree command はログインシェルを経由せず直接実行されるため、ユーザーが追加インストールしたコマンド（`opencode`, `claude` 等）が見つからず `exit_code: -1` で失敗する。

`apply_command` / `archive_command` 等は既に `agent/runner.rs` の `build_command()` で `$SHELL -l -c` パターンを使用しており問題ない。同じパターンを未対応の子プロセス起動箇所にも適用し、一貫性を持たせる。

## What Changes

- `src/server/api.rs` の `run_resolve_command()` を `$SHELL -l -c` 経由に変更
- `src/hooks.rs` の `execute_hook_command()` を `/bin/sh -c` から `$SHELL -l -c` に変更
- `src/web/api.rs` の worktree command 実行を `$SHELL -l -c` 経由に変更
- Windows では既存の `cmd /C` 動作を維持

## Impact

- Affected specs: process-execution
- Affected code: `src/server/api.rs`, `src/hooks.rs`, `src/web/api.rs`
