## Implementation Tasks

- [x] Add `max_retries: u32` (default 0) and `retry_delay_secs: u64` (default 3) to `HookConfig` in `src/hooks.rs` (verification: `cargo test test_hooks_config_deserialize` passes with new fields)
- [ ] Add `repo_root: PathBuf` field to `HookRunner` and update all constructors (`new`, `with_event_tx`, `with_output_handler`) to accept `repo_root` parameter (verification: `cargo build` succeeds)
- [ ] Set `cmd.current_dir(&self.repo_root)` in `execute_hook()` (verification: unit test confirms cwd is repo_root)
- [ ] Update all `HookRunner` instantiation sites to pass `repo_root`: `src/orchestrator.rs`, `src/parallel_run_service.rs`, `src/tui/orchestrator.rs`, `src/tui/command_handlers.rs` (verification: `cargo build` succeeds)
- [ ] Add `index_lock_wait_secs: u64` (default 10) to `HooksConfig` in `src/config/types.rs` (verification: `cargo test` config deserialization tests pass)
- [ ] Implement `wait_for_index_lock_release()` method in `HookRunner` that polls `.git/index.lock` every 500ms up to `index_lock_wait_secs` (verification: unit test with temp lock file)
- [ ] Call `wait_for_index_lock_release()` before executing `on_merged` hook in `run_hook()` (verification: integration test confirms wait behavior)
- [ ] Add retry loop in `run_hook()`: on non-zero exit, retry up to `max_retries` times with `retry_delay_secs` delay, then apply `continue_on_failure` logic (verification: unit test with mock command that fails then succeeds)
- [ ] Add deserialization tests for `HookConfig` with `max_retries` and `retry_delay_secs` fields (verification: `cargo test test_hooks_config` passes)
- [ ] Add test for backward compatibility: existing string and object hook configs without new fields parse correctly with defaults (verification: `cargo test` passes)
- [ ] Run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` (verification: all pass)

## Future Work

- Consider a batch mode for on_merged that runs once after all merges complete
- Monitor whether `cargo release --allow-dirty` is a safer alternative for the bump script
