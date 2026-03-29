# Change: ACP spawn でログインシェル PATH を使ってコマンドを解決する

**Change Type**: implementation

## Why

`AcpClient::spawn()` は `tokio::process::Command::new(&config.acp_command)` で ACP コマンドを直接実行する。cflx が非ログインシェル環境（launchd, systemd, cron 等）から起動された場合、ユーザ固有の PATH（`~/.bun/bin`, `~/.cargo/bin` 等）が含まれず `opencode` が見つからない。

同プロジェクトの `resolve_command` や `hooks` は `shell_command::build_login_shell_command()` で対処済みだが、ACP は stdin/stdout パイプで JSON-RPC 通信するため同じ手法が使えない。

## What Changes

- `AcpClient::spawn()` で `acp_command` が相対パス（`/` で始まらない）の場合、spawn 前にログインシェル経由で `which` を実行して絶対パスに解決する
- 絶対パスの場合はそのまま使用する
- 解決に失敗した場合は元のコマンド名でフォールバック（従来と同じ動作）

## Impact

- Affected specs: `proposal-session-backend`
- Affected code: `src/server/acp_client.rs`
