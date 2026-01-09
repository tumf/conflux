# OpenSpec Orchestrator

Automates the OpenSpec change workflow: list → dependency analysis → apply → archive.

## Features

- 🤖 **Automated Workflow**: Automatically processes OpenSpec changes from detection to archival
- 🧠 **LLM Dependency Analysis**: Uses OpenCode to intelligently analyze and order changes
- 📊 **Real-time Progress**: Visual progress bars showing overall and per-change status
- 💾 **State Management**: Persistent state for recovery and resumption
- 🔍 **Headless Execution**: Uses `opencode run` for autonomous, non-interactive processing

## Architecture

```
┌─────────────────────────────────────────────┐
│     openspec-orchestrator (Rust CLI)        │
├─────────────────────────────────────────────┤
│  CLI → Orchestrator → State Manager         │
│    ↓        ↓              ↓                │
│  OpenSpec  OpenCode    Progress Display     │
└─────────────────────────────────────────────┘
```

## Installation

### Build from source

```bash
cd openspec-orchestrator
cargo build --release
```

The binary will be available at `target/release/openspec-orchestrator`.

### Add to PATH (optional)

```bash
cargo install --path .
```

## Usage

### Run orchestration

Process all pending changes:

```bash
openspec-orchestrator run
```

Process a specific change:

```bash
openspec-orchestrator run --change add-feature-x
```

Dry run (preview without execution):

```bash
openspec-orchestrator run --dry-run
```

Custom binary paths:

```bash
openspec-orchestrator run \
  --opencode-path /usr/local/bin/opencode \
  --openspec-path /usr/local/bin/openspec
```

### Check status

View current orchestration state:

```bash
openspec-orchestrator status
```

### Reset state

Reset orchestration state (with confirmation):

```bash
openspec-orchestrator reset
```

Skip confirmation:

```bash
openspec-orchestrator reset --yes
```

## How It Works

### Main Loop

```
1. List changes via openspec list
   ↓
2. Select next change
   • Priority 1: 100% complete (ready for archive)
   • Priority 2: LLM dependency analysis
   • Priority 3: Highest progress (fallback)
   ↓
3. Process change
   • If complete: openspec archive
   • If incomplete: opencode run "/openspec-apply <id>"
   ↓
4. Update state and repeat
```

### Dependency Analysis

The orchestrator uses OpenCode to analyze dependencies:

```rust
// Prompt sent to LLM
"以下のOpenSpec変更から、次に実行すべきものを1つ選んでください。

変更一覧:
- add-feature-x (2/5 tasks, 40.0%)
- fix-bug-y (5/5 tasks, 100.0%)
- refactor-z (0/3 tasks, 0.0%)

選択基準:
1. 依存関係がない、または依存先が完了しているもの
2. 進捗が進んでいるもの（継続性）
3. 名前から推測される依存関係を考慮

回答は変更IDのみを1行で出力してください。"
```

### State Persistence

State is saved to `.opencode/orchestrator-state.json`:

```json
{
  "current_change": "add-feature-x",
  "processed_changes": ["add-feature-x"],
  "archived_changes": ["fix-bug-y"],
  "failed_changes": [],
  "started_at": "2026-01-08T15:00:00Z",
  "last_update": "2026-01-08T15:45:00Z",
  "total_iterations": 5
}
```

## OpenCode Commands

The orchestrator uses two custom OpenCode commands:

### `/openspec-apply`

Implements the next incomplete task for a change:

```bash
opencode run "/openspec-apply add-feature-x"
```

Behavior:
1. Read `openspec/changes/<id>/tasks.md`
2. Find first incomplete task
3. Implement the task
4. Update `tasks.md` with `[x]`
5. Exit when done

### `/openspec-archive`

Archives a completed change:

```bash
opencode run "/openspec-archive add-feature-x"
```

Behavior:
1. Verify all tasks are complete
2. Run `openspec archive <id> --yes`
3. Report result

## Configuration

### Agent Configuration File (JSONC)

The orchestrator supports configurable agent commands via JSONC configuration files.
This allows you to use different AI tools (OpenCode, Codex, Claude Code, etc.) without code changes.

**Configuration file locations** (in order of priority):
1. `.openspec-orchestrator.jsonc` (project root)
2. `~/.config/openspec-orchestrator/config.jsonc` (global)
3. Custom path via `--config` option

**Example configuration:**

```jsonc
{
  // Apply command template
  // Placeholder: {change_id} - replaced with the change ID at runtime
  "apply_command": "codex run 'openspec-apply {change_id}'",

  // Archive command template
  "archive_command": "codex run 'openspec-archive {change_id}'",

  // Analyze command template
  // Placeholder: {prompt} - replaced with the analysis prompt at runtime
  "analyze_command": "claude '{prompt}'"
}
```

**Default commands** (when no config file is present):

```jsonc
{
  "apply_command": "opencode run '/openspec-apply {change_id}'",
  "archive_command": "opencode run '/openspec-archive {change_id}'",
  "analyze_command": "opencode run --format json '{prompt}'"
}
```

**Placeholders:**

| Placeholder | Description | Used in |
|-------------|-------------|---------|
| `{change_id}` | The change ID being processed | apply_command, archive_command |
| `{prompt}` | LLM analysis prompt | analyze_command |

**Quick start:**

```bash
# Copy the example configuration
cp .openspec-orchestrator.jsonc.example .openspec-orchestrator.jsonc

# Edit to use your preferred agent
vim .openspec-orchestrator.jsonc

# Run with the configuration
openspec-orchestrator run
```

**Use a custom config path:**

```bash
openspec-orchestrator run --config /path/to/config.jsonc
```

### Hooks Configuration

You can configure hooks to run commands at various stages of the orchestration process.
Hooks are defined in the `hooks` section of the configuration file.

```jsonc
{
  "hooks": {
    // Simple string format (uses default settings)
    "on_start": "echo 'Orchestrator started'",
    
    // Object format (with detailed settings)
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": false,  // Stop orchestration if command fails
      "timeout": 300                 // Timeout in seconds
    },
    
    // Available hooks:
    "on_first_apply": "git checkout -b feature/orchestrator-run",
    "pre_apply": "echo 'Applying {change_id}'",
    "on_change_complete": "echo '{change_id} is 100% complete'",
    "pre_archive": "cargo clippy -- -D warnings",
    "post_archive": "echo '{change_id} archived successfully'",
    "on_finish": "echo 'Finished with status: {status}'",
    "on_error": "echo 'Error in {change_id}: {error}' >> errors.log"
  }
}
```

**Available Hooks:**

| Hook Name | Trigger | Description |
|-----------|---------|-------------|
| `on_start` | Start | Orchestrator starts |
| `on_first_apply` | Before First Apply | Triggered only once before the first apply |
| `on_iteration_start` | Iteration Start | Start of each iteration loop |
| `pre_apply` | Before Apply | Before applying a change |
| `post_apply` | After Apply | After successfully applying a change |
| `on_change_complete` | Task 100% | When a change reaches 100% task completion |
| `pre_archive` | Before Archive | Before archiving a change |
| `post_archive` | After Archive | After successfully archiving a change |
| `on_iteration_end` | Iteration End | End of each iteration loop |
| `on_queue_change` | Queue Change | When the number of pending changes changes |
| `on_finish` | Finish | Orchestrator completes (success or limit) |
| `on_error` | Error | When an error occurs during apply or archive |

**Placeholders:**

| Placeholder | Description |
|-------------|-------------|
| `{change_id}` | Current Change ID |
| `{iteration}` | Current iteration number |
| `{total_changes}` | Total number of changes in initial snapshot |
| `{queue_size}` | Current queue size |
| `{completed_tasks}` | Number of completed tasks for current change |
| `{total_tasks}` | Total number of tasks for current change |
| `{status}` | Finish status (completed/iteration_limit) |
| `{error}` | Error message |

**Environment Variables:**

Hooks receive context via environment variables:
`OPENSPEC_CHANGE_ID`, `OPENSPEC_ITERATION`, `OPENSPEC_TOTAL_CHANGES`, `OPENSPEC_QUEUE_SIZE`, `OPENSPEC_COMPLETED_TASKS`, `OPENSPEC_TOTAL_TASKS`, `OPENSPEC_STATUS`, `OPENSPEC_ERROR`, `OPENSPEC_DRY_RUN`

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENSPEC_CMD` | OpenSpec command (can include arguments) | `npx @fission-ai/openspec@latest` |
| `RUST_LOG` | Logging level | (none) |

Example:

```bash
# Use a custom openspec installation
export OPENSPEC_CMD="/usr/local/bin/openspec"
openspec-orchestrator run

# Use a specific version via npx
export OPENSPEC_CMD="npx @fission-ai/openspec@1.2.3"
openspec-orchestrator run
```

### Command-line Options

```
Options:
  --change <ID>             Process only specified change
  --config <PATH>           Custom configuration file path (JSONC)
  --openspec-cmd <CMD>      Custom openspec command [env: OPENSPEC_CMD]
```

Priority: CLI argument > Environment variable > Default value

## Error Handling

| Error | Behavior |
|-------|----------|
| OpenCode startup fails | Retry 3 times, then mark as failed |
| Apply command fails | Mark change as failed, continue with others |
| Archive command fails | Mark change as failed, continue with others |
| LLM analysis fails | Fall back to progress-based selection |
| All changes fail | Exit with error |

## Troubleshooting

### "No changes found"

- Run `openspec list` to verify changes exist
- Check that you're in the correct directory

### "OpenCode command failed"

- Verify `opencode` is installed: `which opencode`
- Test manually: `opencode run "echo test"`
- Check OpenCode configuration: `~/.config/opencode/opencode.jsonc`

### "All changes failed"

- Check logs for specific errors
- Review `.opencode/orchestrator-state.json`
- Try processing a single change: `--change <id>`
- Use dry run to preview: `--dry-run`

### State corruption

Reset state and restart:

```bash
openspec-orchestrator reset --yes
openspec-orchestrator run
```

## Development

### Run tests

```bash
cargo test
```

### Run tests with coverage

```bash
# Install cargo-llvm-cov if not present
cargo install cargo-llvm-cov

# Run tests with coverage summary
cargo llvm-cov --all-features

# Generate detailed HTML report (opens in browser)
cargo llvm-cov --all-features --html --open

# Generate JSON report for CI/CD
cargo llvm-cov --all-features --json --output-path coverage.json

# Show coverage for specific module
cargo llvm-cov --all-features -- --test-threads=1 2>&1 | grep -A 5 "src/config.rs"
```

### Run with logging

```bash
RUST_LOG=debug cargo run -- run --dry-run
```

### Project Structure

```
src/
├── main.rs           # Entry point
├── cli.rs            # CLI argument parsing
├── config.rs         # Configuration file parsing (JSONC)
├── agent.rs          # Agent runner (configurable commands)
├── error.rs          # Error types
├── openspec.rs       # OpenSpec wrapper (list, archive)
├── opencode.rs       # OpenCode runner (legacy, kept for compatibility)
├── progress.rs       # Progress display (indicatif)
├── tui.rs            # Interactive TUI dashboard
└── orchestrator.rs   # Main orchestration loop
```

## Future Enhancements

- [ ] Parallel execution for independent changes
- [ ] Slack/Discord notifications
- [ ] Maximum iteration limit (prevent infinite loops)
- [ ] Manual priority override
- [ ] Enhanced dry-run with execution plan
- [ ] Web UI for monitoring

## License

MIT

## Contributing

Contributions welcome! Please open an issue or pull request.
