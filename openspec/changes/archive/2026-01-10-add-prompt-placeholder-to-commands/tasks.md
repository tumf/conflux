# Tasks

## Phase 1: Core Implementation

- [x] 1.1 Update `OrchestratorConfig` in `src/config.rs`
  - Add `apply_prompt` field (Option<String>)
  - Add `archive_prompt` field (Option<String>)
  - Add `get_apply_prompt()` method (returns default if not set)
  - Add `get_archive_prompt()` method (returns default if not set)
  - Define `DEFAULT_APPLY_PROMPT` constant
  - Define `DEFAULT_ARCHIVE_PROMPT` constant (empty string)

- [x] 1.2 Update `AgentRunner` in `src/agent.rs`
  - Modify `run_apply_streaming()` to expand both `{change_id}` and `{prompt}`
  - Modify `run_apply()` to expand both placeholders
  - Modify `run_archive_streaming()` to expand both placeholders
  - Modify `run_archive()` to expand both placeholders

## Phase 2: Templates and Documentation

- [x] 2.1 Update templates in `src/templates.rs`
  - Add `{prompt}` placeholder to `apply_command` in CLAUDE_TEMPLATE
  - Add `{prompt}` placeholder to `archive_command` in CLAUDE_TEMPLATE
  - Add `{prompt}` placeholder to `apply_command` in OPENCODE_TEMPLATE
  - Add `{prompt}` placeholder to `archive_command` in OPENCODE_TEMPLATE
  - Add `{prompt}` placeholder to `apply_command` in CODEX_TEMPLATE
  - Add `{prompt}` placeholder to `archive_command` in CODEX_TEMPLATE
  - Add `apply_prompt` and `archive_prompt` default values to templates

- [x] 2.2 Update documentation
  - Update README.md placeholder table
  - Update README.ja.md placeholder table
  - Add examples for `apply_prompt` and `archive_prompt` configuration

## Phase 3: Testing

- [x] 3.1 Add unit tests in `src/config.rs`
  - Test `get_apply_prompt()` default value
  - Test `get_archive_prompt()` default value
  - Test custom prompt values

- [x] 3.2 Add unit tests in `src/agent.rs`
  - Test apply command with prompt expansion
  - Test archive command with prompt expansion
  - Test commands with both `{change_id}` and `{prompt}`

- [x] 3.3 Update E2E tests if needed
  - Verify placeholder expansion in integration tests

## Validation

- [x] Run `cargo build` - no errors
- [x] Run `cargo test` - all tests pass
- [x] Run `cargo clippy` - no new warnings (existing warning unrelated to this change)
- [x] Run `openspec validate add-prompt-placeholder-to-commands --strict`
