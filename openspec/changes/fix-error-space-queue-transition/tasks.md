## Implementation Tasks

- [ ] Task 1: `update_change_status()` の L63 ガードから `"error"` を除外する (verification: `cargo test` で既存テストが通り、error→queued 遷移が許可されることを確認)
- [ ] Task 2: `handle_toggle_running_mode()` の `"error"` アームで `selected=true` 時に `TuiCommand::AddToQueue`、`selected=false` 時に `TuiCommand::RemoveFromQueue` を発行する (verification: `cargo test test_running_mode_error_change_toggle` で AddToQueue コマンドが返ることを確認)
- [ ] Task 3: 既存テスト `test_running_mode_error_change_toggle_sets_retry_mark` を更新し、`ToggleActionResult::Command(AddToQueue)` を期待するように変更する (verification: `cargo test test_running_mode_error_change_toggle`)
- [ ] Task 4: 新規テスト追加: Running モードで error change に Space→再度 Space で AddToQueue→RemoveFromQueue の順でコマンドが発行されることを検証する (verification: `cargo test test_running_mode_error_change_toggle_queue`)
- [ ] Task 5: `update_change_status` ガード変更のリグレッションテスト: archived/merged→queued がブロックされることを確認する (verification: `cargo test update_change_status`)
- [ ] Task 6: `cargo clippy -- -D warnings && cargo fmt --check` で lint/format チェック (verification: exit code 0)

## Future Work

- なし
