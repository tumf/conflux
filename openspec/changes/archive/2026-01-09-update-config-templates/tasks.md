# Tasks

## Implementation Tasks

- [x] Update `CLAUDE_TEMPLATE` in `src/templates.rs`:
  - Remove `agent` wrapper, use flat structure
  - Add `analyze_command` with `--verbose --output-format stream-json`
  - Add `apply_command` with `--verbose --output-format stream-json`
  - Add `archive_command` with `--verbose --output-format stream-json`

- [x] Update `OPENCODE_TEMPLATE` in `src/templates.rs`:
  - Remove `agent` wrapper, use flat structure
  - Add `archive_command`
  - Add `analyze_command`

- [x] Update `CODEX_TEMPLATE` in `src/templates.rs`:
  - Remove `agent` wrapper, use flat structure
  - Add `archive_command`
  - Add `analyze_command`

- [x] Update unit tests in `templates.rs` to match new structure

## Validation

- [x] Run `cargo test` to verify all tests pass
- [x] Run `cargo clippy` to check for warnings
- [x] Test `cflx init --template claude` generates valid config
- [x] Verify generated config is parseable by `config.rs`

## Dependencies

- Tasks 1-3 can run in parallel
- Task 4 depends on Tasks 1-3
- Tasks 5-8 depend on Task 4
