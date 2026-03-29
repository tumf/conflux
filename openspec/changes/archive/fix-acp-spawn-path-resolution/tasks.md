## Implementation Tasks

- [x] Task 1: Add `resolve_command_path` helper to `src/server/acp_client.rs` that runs `$SHELL -l -c 'which <command>'` to resolve relative command names to absolute paths (verification: `cargo test acp_client` — unit test with `which cat` returns `/bin/cat` or similar absolute path)
- [x] Task 2: Update `AcpClient::spawn()` to call `resolve_command_path` before `Command::new()` when `acp_command` does not start with `/` (verification: `cargo test acp_client` — integration test that spawn with a PATH-dependent command resolves correctly)
- [x] Task 3: Add unit test for fallback behavior — when `which` fails (e.g., non-existent command), `resolve_command_path` returns the original command name unchanged (verification: `cargo test resolve_command_path_fallback`)
- [x] Task 4: Run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` to confirm no regressions
