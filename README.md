# OpenSpec Orchestrator

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Automates the OpenSpec change workflow: list → dependency analysis → apply → archive.

## Features

- 🖥️ **Interactive TUI**: Default mode with real-time progress dashboard
- 🤖 **Automated Workflow**: Automatically processes OpenSpec changes from detection to archival
- 🧠 **LLM Dependency Analysis**: Uses AI agents to intelligently analyze and order changes
- 📊 **Real-time Progress**: Visual progress bars showing overall and per-change status
- 🔌 **Multi-Agent Support**: Works with Claude Code, OpenCode, and Codex
- 🪝 **Lifecycle Hooks**: Configurable hooks for custom actions at each workflow stage
- ✅ **Approval Workflow**: Approve changes with checksum validation before processing
- ⚡ **Parallel Execution**: Process multiple independent changes simultaneously using jj workspaces or Git worktrees
- 🌐 **Web Monitoring**: Optional HTTP server with REST API and WebSocket for remote monitoring

## Architecture

```
┌─────────────────────────────────────────────┐
│     openspec-orchestrator (Rust CLI)        │
├─────────────────────────────────────────────┤
│  CLI → Orchestrator → State Manager         │
│    ↓        ↓              ↓                │
│  OpenSpec  AI Agent    Progress Display     │
│            (Claude/OpenCode/Codex)          │
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

### Default: Interactive TUI

Running without any subcommand launches the interactive TUI dashboard:

```bash
openspec-orchestrator
```

The TUI provides:
- Real-time change status visualization
- Progress tracking for all pending changes
- Keyboard navigation and controls

#### TUI Change States

Changes have two independent states: **approval** and **selection/queue**.

**Checkbox Display:**
| Symbol | State | Description |
|--------|-------|-------------|
| `[ ]` | Unapproved | Cannot be selected for processing |
| `[@]` | Approved (not selected) | Ready to be selected |
| `[x]` | Selected (reserved) | Will be queued when F5 is pressed |

**Queue Status (shown in Running mode):**
| Status | Description |
|--------|-------------|
| `[Queued]` | Waiting to be processed |
| `[Processing]` | Currently being applied |
| `[Completed]` | All tasks finished |
| `[Archived]` | Successfully archived |
| `[Error]` | Processing failed |

**Workflow:**
1. **Select mode**: Use `@` to approve changes, then `Space` to select (reserve) them
2. Press `F5` to start processing - all selected changes become `[Queued]`
3. **Running mode**: Watch progress as changes move through Queued → Processing → Completed → Archived

#### TUI Key Bindings

| Key | Select Mode | Running Mode |
|-----|-------------|--------------|
| `↑/↓` or `j/k` | Navigate list | Navigate list |
| `Space` | Toggle selection | Add/remove from queue |
| `@` | Toggle approval | Toggle approval |
| `e` | Open editor | Open editor |
| `+` | New proposal | New proposal |
| `w` | Show QR code* | Show QR code* |
| `F5` | Start processing | - |
| `=` | Toggle parallel mode | - |
| `Esc` | - | Stop (graceful/force) |
| `q` | Quit | Quit |
| `PageUp/Down` | - | Scroll logs |

*QR code is only available when web monitoring is enabled (`--web` flag). Press any key to close the QR popup.

**Proposal Mode:**
| Key | Action |
|-----|--------|
| `Ctrl+S` | Submit proposal |
| `Esc` | Cancel and return |

### Initialize Configuration

Generate a configuration file for your preferred AI agent:

```bash
# Default: Claude Code template
openspec-orchestrator init

# OpenCode template
openspec-orchestrator init --template opencode

# Codex template
openspec-orchestrator init --template codex

# Overwrite existing config
openspec-orchestrator init --force
```

Available templates: `claude` (default), `opencode`, `codex`

### Run Orchestration (Non-Interactive)

Process all pending changes in headless mode:

```bash
openspec-orchestrator run
```

Process specific changes (single or multiple):

```bash
# Single change
openspec-orchestrator run --change add-feature-x

# Multiple changes (comma-separated)
openspec-orchestrator run --change add-feature-x,fix-bug-y,refactor-z
```

Custom configuration file:

```bash
openspec-orchestrator run --config /path/to/config.jsonc
```

### Launch TUI Explicitly

```bash
openspec-orchestrator tui
```

### Manage Change Approval

Approve or unapprove changes to control which changes can be processed:

```bash
# Approve a change (creates checksums for validation)
openspec-orchestrator approve set add-feature-x

# Check approval status
openspec-orchestrator approve status add-feature-x

# Unapprove a change
openspec-orchestrator approve unset add-feature-x
```

Approved changes have an `approved` file containing MD5 checksums of all specification files (excluding `tasks.md`). This ensures the change hasn't been modified since approval.

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
   • If incomplete: AI agent applies next task
   ↓
4. Update state and repeat
```

### Dependency Analysis

The orchestrator uses an AI agent to analyze dependencies:

```
// Prompt sent to LLM
"Select the next change to execute from the following OpenSpec changes.

Changes:
- add-feature-x (2/5 tasks, 40.0%)
- fix-bug-y (5/5 tasks, 100.0%)
- refactor-z (0/3 tasks, 0.0%)

Selection criteria:
1. No dependencies, or dependencies are completed
2. Higher progress (continuity)
3. Consider dependencies inferred from names

Output only the change ID on a single line."
```

## Configuration

### Agent Configuration File (JSONC)

The orchestrator supports configurable agent commands via JSONC configuration files.
This allows you to use different AI tools (Claude Code, OpenCode, Codex, etc.) without code changes.

**Configuration file locations** (in order of priority):
1. `.openspec-orchestrator.jsonc` (project root)
2. `~/.config/openspec-orchestrator/config.jsonc` (global)
3. Custom path via `--config` option

**Example configuration (Claude Code):**

```jsonc
{
  // Command to analyze dependencies and select next change
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // Command to apply a change (supports {change_id} and {prompt} placeholders)
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",

  // Command to archive a completed change (supports {change_id} and {prompt} placeholders)
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  // System prompt for apply command (injected into {prompt} placeholder)
  "apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。",

  // System prompt for archive command (injected into {prompt} placeholder)
  "archive_prompt": "",

  // Command to propose new changes from TUI (+ key)
  // Supports {proposal} placeholder for the proposal text
  "propose_command": "claude --dangerously-skip-permissions --verbose -p '/openspec:proposal {proposal}'",

  // Lifecycle hooks (optional)
  "hooks": {
    // "pre_apply": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Completed {change_id}'"
  }
}
```

**Placeholders:**

| Placeholder | Description | Used in |
|-------------|-------------|---------|
| `{change_id}` | The change ID being processed | apply_command, archive_command |
| `{prompt}` | System prompt for agent commands | apply_command, archive_command, analyze_command |
| `{proposal}` | Proposal text from TUI input | propose_command |

**System Prompts:**

| Config Key | Description | Default |
|------------|-------------|---------|
| `apply_prompt` | Prompt injected into apply_command's `{prompt}` | `スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。` |
| `archive_prompt` | Prompt injected into archive_command's `{prompt}` | (empty) |

**Quick start:**

```bash
# Generate configuration with init command
openspec-orchestrator init

# Or copy the example configuration
cp .openspec-orchestrator.jsonc.example .openspec-orchestrator.jsonc

# Edit to customize settings
vim .openspec-orchestrator.jsonc

# Run with the configuration
openspec-orchestrator
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

    // Run lifecycle hooks
    "on_start": "echo 'Starting orchestration with {total_changes} changes'",
    "on_finish": "echo 'Finished with status: {status}'",
    "on_error": "echo 'Error in {change_id}: {error}' >> errors.log",

    // Change lifecycle hooks
    "on_change_start": "echo 'Starting {change_id}'",
    "pre_apply": "echo 'Applying {change_id} (attempt {apply_count})'",
    "post_apply": "cargo test",
    "on_change_complete": "echo '{change_id} is 100% complete'",
    "pre_archive": "cargo clippy -- -D warnings",
    "post_archive": "echo '{change_id} archived successfully'",
    "on_change_end": "echo 'Finished processing {change_id}'",

    // TUI-only hooks (user interaction)
    "on_queue_add": "echo 'Added {change_id} to queue'",
    "on_queue_remove": "echo 'Removed {change_id} from queue'",
    "on_approve": "echo 'Approved {change_id}'",
    "on_unapprove": "echo 'Unapproved {change_id}'"
  }
}
```

**Available Hooks:**

*Run lifecycle hooks:*

| Hook Name | Trigger | Description |
|-----------|---------|-------------|
| `on_start` | Start | Orchestrator starts |
| `on_finish` | Finish | Orchestrator completes (success or limit) |
| `on_error` | Error | When an error occurs during apply or archive |

*Change lifecycle hooks:*

| Hook Name | Trigger | Description |
|-----------|---------|-------------|
| `on_change_start` | Change Start | When processing begins for a new change |
| `pre_apply` | Before Apply | Before applying a change |
| `post_apply` | After Apply | After successfully applying a change |
| `on_change_complete` | Task 100% | When a change reaches 100% task completion |
| `pre_archive` | Before Archive | Before archiving a change |
| `post_archive` | After Archive | After successfully archiving a change |
| `on_change_end` | Change End | After a change is successfully archived |

*TUI-only hooks (user interaction):*

| Hook Name | Trigger | Description |
|-----------|---------|-------------|
| `on_queue_add` | Queue Add | When user adds a change to queue (Space key) |
| `on_queue_remove` | Queue Remove | When user removes a change from queue (Space key) |
| `on_approve` | Approve | When user approves a change (@ key) |
| `on_unapprove` | Unapprove | When user unapproves a change (@ key) |

**Placeholders:**

| Placeholder | Description |
|-------------|-------------|
| `{change_id}` | Current Change ID |
| `{changes_processed}` | Number of changes processed so far |
| `{total_changes}` | Total number of changes in initial snapshot |
| `{remaining_changes}` | Remaining changes in queue |
| `{apply_count}` | Number of apply attempts for current change |
| `{completed_tasks}` | Number of completed tasks for current change |
| `{total_tasks}` | Total number of tasks for current change |
| `{status}` | Finish status (completed/iteration_limit) |
| `{error}` | Error message |

**Environment Variables:**

Hooks receive context via environment variables:
`OPENSPEC_CHANGE_ID`, `OPENSPEC_CHANGES_PROCESSED`, `OPENSPEC_TOTAL_CHANGES`, `OPENSPEC_REMAINING_CHANGES`, `OPENSPEC_APPLY_COUNT`, `OPENSPEC_COMPLETED_TASKS`, `OPENSPEC_TOTAL_TASKS`, `OPENSPEC_STATUS`, `OPENSPEC_ERROR`, `OPENSPEC_DRY_RUN`

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENSPEC_CMD` | OpenSpec command (can include arguments) | `npx @fission-ai/openspec@latest` |
| `RUST_LOG` | Logging level | (none) |

Example:

```bash
# Use a custom openspec installation
export OPENSPEC_CMD="/usr/local/bin/openspec"
openspec-orchestrator

# Use a specific version via npx
export OPENSPEC_CMD="npx @fission-ai/openspec@1.2.3"
openspec-orchestrator
```

### Command-line Options

```
Usage: openspec-orchestrator [OPTIONS] [COMMAND]

Commands:
  run      Run the OpenSpec change orchestration loop (non-interactive)
  tui      Launch the interactive TUI dashboard
  init     Initialize a new configuration file
  approve  Manage change approval status

Options:
  --opencode-path <PATH>   Path to opencode binary (deprecated, use config file)
  --openspec-cmd <CMD>     OpenSpec command [env: OPENSPEC_CMD]
  -h, --help               Print help
```

**Run subcommand options:**
```
Options:
  --change <ID,...>     Process only specified changes (comma-separated)
  -c, --config <PATH>   Custom configuration file path (JSONC)
  --openspec-cmd <CMD>  Custom openspec command [env: OPENSPEC_CMD]
  --parallel            Enable parallel execution mode
  --max-concurrent <N>  Maximum concurrent workspaces (default: 3)
  --vcs <BACKEND>       VCS backend: auto, jj, or git (default: auto)
  --no-resume           Disable workspace resume (always create new workspaces)
  --dry-run             Preview parallelization groups without executing
  --web                 Enable web monitoring server
  --web-port <PORT>     Web server port (default: 8080)
  --web-bind <ADDR>     Web server bind address (default: 127.0.0.1)
```

### Parallel Execution

The orchestrator supports parallel execution of independent changes using either jj workspaces or Git worktrees.

**VCS Backend Selection:**

| Backend | Description | Requirements |
|---------|-------------|--------------|
| `auto` | Auto-detect (jj preferred, then Git) | Either jj or Git repository |
| `jj` | Use jj workspaces | jj repository (.jj directory) |
| `git` | Use Git worktrees | Git repository with clean working directory |

**Usage:**

```bash
# Auto-detect VCS backend (default)
openspec-orchestrator run --parallel

# Force Git worktrees
openspec-orchestrator run --parallel --vcs git

# Force jj workspaces
openspec-orchestrator run --parallel --vcs jj

# Preview parallelization groups without executing
openspec-orchestrator run --parallel --dry-run

# Limit concurrent workspaces
openspec-orchestrator run --parallel --max-concurrent 5
```

**Configuration:**

You can also set the VCS backend in your configuration file:

```jsonc
{
  // VCS backend for parallel execution: "auto", "jj", or "git"
  "vcs_backend": "auto",

  // Maximum concurrent workspaces
  "max_concurrent_workspaces": 3
}
```

**Git Requirements:**

When using Git worktrees:
- Working directory must be clean (no uncommitted changes)
- Each change runs in an isolated worktree with its own branch
- Changes are merged back sequentially after completion

**jj Requirements:**

When using jj workspaces:
- Working copy changes are automatically snapshotted
- Each change runs in an isolated workspace
- Changes are merged using jj's conflict-free merge

**Workspace Resume:**

By default, the orchestrator automatically detects and reuses existing workspaces from interrupted runs. This allows you to resume work from where you left off without losing progress.

- When a workspace is found for a change ID, it is reused instead of creating a new one
- If multiple workspaces exist for the same change, the newest one is used and older ones are cleaned up
- Use `--no-resume` to disable this behavior and always create fresh workspaces

```bash
# Resume from existing workspaces (default behavior)
openspec-orchestrator run --parallel

# Always create new workspaces (discard any existing work)
openspec-orchestrator run --parallel --no-resume
```

### Web Monitoring (Optional Feature)

The orchestrator supports an optional HTTP server for remote monitoring of orchestration progress via web browser.

**Note:** Web monitoring requires compiling with the `web-monitoring` feature flag:

```bash
cargo build --release --features web-monitoring
```

**Usage:**

```bash
# Enable web monitoring on default port (8080)
openspec-orchestrator run --web

# Custom port and bind address
openspec-orchestrator run --web --web-port 9000 --web-bind 0.0.0.0

# With TUI mode
openspec-orchestrator tui --web
```

**Features:**

- **Dashboard UI**: View progress at `http://localhost:8080/`
- **Real-time updates**: WebSocket connection for live progress updates
- **REST API**: Query state programmatically
- **QR Code Popup**: Press `w` in the TUI to display a QR code for quick mobile access to the dashboard

**REST API Endpoints:**

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/health` | GET | Health check |
| `/api/state` | GET | Full orchestrator state |
| `/api/changes` | GET | List all changes with progress |
| `/api/changes/{id}` | GET | Details for a specific change |

**WebSocket:**

Connect to `ws://localhost:8080/ws` for real-time state updates. Messages are JSON with the following format:

```json
{
  "type": "state_update",
  "timestamp": "2024-01-12T10:30:00Z",
  "changes": [
    {
      "id": "add-feature",
      "completed_tasks": 3,
      "total_tasks": 10,
      "progress_percent": 30.0,
      "status": "in_progress"
    }
  ]
}
```

**Dashboard Overview:**

The web dashboard provides a visual overview of orchestration progress:

```
┌─────────────────────────────────────────────────────────────────┐
│  OpenSpec Orchestrator                           ● Connected    │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│  │    5    │  │    2    │  │    1    │  │    2    │            │
│  │  Total  │  │Complete │  │Progress │  │ Pending │            │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ add-feature-auth                    [APPROVED] [IN_PROGRESS]│
│  │ ████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  40%    │   │
│  │ 4/10 tasks                                               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ fix-login-bug                       [APPROVED] [COMPLETE]   │
│  │ ████████████████████████████████████████████████  100%  │   │
│  │ 5/5 tasks                                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ refactor-api                        [PENDING]               │
│  │ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  0%    │   │
│  │ 0/8 tasks                                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  Last updated: 2024-01-12 10:30:00                              │
└─────────────────────────────────────────────────────────────────┘
```

**Dashboard Features:**

- **Stats Bar**: Shows total, completed, in-progress, and pending change counts
- **Change Cards**: Each change displays ID, approval status, progress status, and progress bar
- **Real-time Updates**: Progress updates automatically via WebSocket connection
- **Connection Status**: Shows current WebSocket connection state (Connected/Disconnected)
- **Responsive Design**: Works on desktop and mobile browsers

**Web Monitoring Troubleshooting:**

| Issue | Solution |
|-------|----------|
| "Address already in use" | Another process is using port 8080. Use `--web-port` to specify a different port |
| Dashboard not loading | Ensure `--web` flag is enabled. Verify the URL includes the correct port |
| WebSocket disconnects frequently | Check network stability. The dashboard auto-reconnects on disconnection |
| Changes not updating | Refresh the page or check that the orchestrator is actively processing |
| Cannot access from another device | Use `--web-bind 0.0.0.0` to allow external connections (local network only) |
| CORS errors in browser console | This is normal for cross-origin requests; the server handles CORS headers |

**Init subcommand options:**
```
Options:
  -t, --template <TEMPLATE>  Template to use [default: claude] [possible values: claude, opencode, codex]
  -f, --force                Overwrite existing configuration file
```

**Approve subcommand:**
```
Commands:
  set     Approve a change (create approved file with checksums)
  unset   Unapprove a change (remove approved file)
  status  Check approval status of a change
```

Priority: CLI argument > Environment variable > Default value

## Error Handling

| Error | Behavior |
|-------|----------|
| Agent command fails | Retry 3 times, then mark as failed |
| Apply command fails | Mark change as failed, continue with others |
| Archive command fails | Mark change as failed, continue with others |
| LLM analysis fails | Fall back to progress-based selection |
| All changes fail | Exit with error |

## Troubleshooting

### "No changes found"

- Run `openspec list` to verify changes exist
- Check that you're in the correct directory

### "Agent command failed"

- Verify your AI agent is installed (e.g., `which claude`)
- Test manually: `claude -p "echo test"`
- Check your configuration file: `.openspec-orchestrator.jsonc`

### "All changes failed"

- Check logs for specific errors
- Try processing a single change: `--change <id>`

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for build instructions, testing, and project structure.

## Future Enhancements

- [ ] State persistence for recovery and resumption
- [x] Parallel execution for independent changes (using jj workspaces or Git worktrees)
- [ ] Slack/Discord notifications
- [ ] Maximum iteration limit (prevent infinite loops)
- [ ] Manual priority override
- [ ] Enhanced dry-run with execution plan
- [ ] Web UI for monitoring

## License

MIT

## Contributing

Contributions welcome! Please open an issue or pull request.
