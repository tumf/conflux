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

### Environment Variables

- `RUST_LOG`: Set logging level (e.g., `RUST_LOG=debug`)

### Command-line Options

```
Options:
  --dry-run                 Preview without execution
  --change <ID>             Process only specified change
  --opencode-path <PATH>    Custom opencode binary path
  --openspec-path <PATH>    Custom openspec binary path
```

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

### Run with logging

```bash
RUST_LOG=debug cargo run -- run --dry-run
```

### Project Structure

```
src/
├── main.rs           # Entry point
├── cli.rs            # CLI argument parsing
├── error.rs          # Error types
├── openspec.rs       # OpenSpec wrapper (list, archive)
├── opencode.rs       # OpenCode runner (headless execution)
├── state.rs          # State persistence
├── progress.rs       # Progress display (indicatif)
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
