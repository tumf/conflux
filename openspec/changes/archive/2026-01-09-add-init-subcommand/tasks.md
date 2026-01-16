# Tasks: add-init-subcommand

## Implementation Tasks

- [x] **1. Add Init subcommand to CLI** (`src/cli.rs`)
  - Add `Init` variant to `Commands` enum
  - Add `InitArgs` struct with `--template` and `--force` options
  - Template enum: `opencode`, `claude`, `codex` (default: `claude`)

- [x] **2. Create template module** (`src/templates.rs`)
  - Define template constants for each agent type
  - opencode: Uses `opencode run '/openspec-apply {change_id}'`
  - claude: Uses `claude --dangerously-skip-permissions -p '/openspec:apply {change_id}'`
  - codex: Uses `codex '/openspec:apply {change_id}'`

- [x] **3. Implement init command logic** (`src/main.rs`)
  - Check if `.cflx.jsonc` exists
  - If exists and no `--force`: exit with error
  - If exists and `--force`: overwrite
  - Write selected template to file

- [x] **4. Add tests** (`src/cli.rs`, `src/templates.rs`)
  - Test CLI argument parsing for init subcommand
  - Test template generation for each agent type
  - Test file creation and overwrite behavior

## Verification

- [x] Run `cargo test` to verify all tests pass
- [x] Run `cargo clippy` to ensure no linting issues
- [x] Manual test: `cflx init --template claude`
- [x] Manual test: `cflx init --template opencode`
- [x] Manual test: `cflx init` (default template)
