# Conflux

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Automates the OpenSpec change workflow (list → dependency analysis → apply → archive). Orchestrates `openspec` and AI coding agents to process changes autonomously.

## Features

- 🖥️ **Interactive TUI**: Default mode with real-time progress dashboard
- 🤖 **Automated Workflow**: Automatically processes OpenSpec changes from detection to archival
- 🧠 **LLM Dependency Analysis**: Uses AI agents to intelligently analyze and order changes
- 📊 **Real-time Progress**: Visual progress bars showing overall and per-change status
- 🔌 **Multi-Agent Support**: Works with Claude Code, OpenCode, and Codex
- 🪝 **Lifecycle Hooks**: Configurable hooks for custom actions at each workflow stage
- ✅ **Approval Workflow**: Approve changes with checksum validation before processing
- ⚡ **Parallel Execution**: Process multiple independent changes simultaneously using Git worktrees
- 🌐 **Web Monitoring**: Optional HTTP server with REST API and WebSocket for remote monitoring

## Architecture

```
┌─────────────────────────────────────────────┐
│     cflx (Rust CLI)        │
├─────────────────────────────────────────────┤
│  CLI → Orchestrator → State Manager         │
│    ↓        ↓              ↓                │
│  OpenSpec  AI Agent    Progress Display     │
│            (Claude/OpenCode/Codex)          │
└─────────────────────────────────────────────┘
```

## Usage

### Golden Path: Quick Start

```bash
# Step 1: Generate configuration for your AI agent (Claude Code by default)
cflx init

# Step 2: Edit the generated .cflx.jsonc to configure your agent
vim .cflx.jsonc

# Step 3a: Launch the interactive TUI to review and process changes
cflx

# Step 3b: Or run in headless (non-interactive) mode
cflx run
```

### Interactive TUI (Primary Interface)

The primary way to use the orchestrator is through the interactive TUI dashboard:

```bash
cflx
```

The TUI provides:
- Real-time change status visualization
- Progress tracking for all pending changes
- Keyboard navigation and controls
- Worktree management view

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
| `[not queued]` | Not in the execution queue (can be toggled dynamically while running) |
| `[queued]` | Waiting to be processed |
| `[blocked]` | Blocked by unresolved dependencies |
| `[merge wait]` | Waiting for merge resolution (use `M` to trigger resolve) |
| `[resolve pending]` | Resolve requested, waiting for execution to start (UI actions are restricted) |
| `[applying]` | Applying (spinner + progress% / iteration when available) |
| `[accepting]` | Acceptance/tests (spinner, iteration when available) |
| `[archiving]` | Archiving (spinner, iteration when available) |
| `[resolving]` | Resolving (spinner, iteration when available) |
| `[archived]` | Archived successfully |
| `[merged]` | Merged to base branch (parallel mode only) |
| `[error]` | Processing failed |

**Workflow:**
1. **Select mode (header shows `[Ready]`)**: Use `@` to approve changes, then `Space` to toggle the execution mark (`selected`) only
2. Press `F5` to start processing - all execution-marked changes become `queued`
3. **Running mode (header shows `[Running N]`)**: `queued` → `applying` → (optional `accepting`) → `archiving` → `archived` (parallel mode may also show `merge wait`/`resolving`/`merged`)

#### Header Status

| Display | Meaning |
|---------|---------|
| `[Ready]` | Selection/idle (`AppMode::Select`) |
| `[Running N]` | Active processing, where N counts `applying`/`accepting`/`archiving`/`resolving` |

#### TUI Key Bindings

**Changes View:**

| Key | Select (`[Ready]`) | Running (/Stopping) | Stopped (/Error) |
|-----|-------------------|--------------------|------------------|
| `↑/↓` or `j/k` | Navigate list | Navigate list | Navigate list |
| `Tab` | Switch to Worktrees view | Switch to Worktrees view | Switch to Worktrees view |
| `Space` | Toggle execution mark only | Add/remove from dynamic queue (`not queued`⇄`queued`) | Toggle execution mark for `not queued` only |
| `@` | Toggle approval | Toggle approval | Toggle approval |
| `e` | Open editor | Open editor | Open editor |
| `w` | Show QR code* | Show QR code* | Show QR code* |
| `M` | Resolve when status is `merge wait` | Resolve when status is `merge wait` | Resolve when status is `merge wait` |
| `F5` | Start processing | (In Stopping: cancel stop) | Resume (Stopped) / Retry (Error) |
| `=` | Toggle parallel mode | - | Toggle parallel mode |
| `Esc` | - | Stop (1st: graceful, 2nd: force) | - |
| `PageUp/Down` | (When logs are shown) scroll logs | Scroll logs | Scroll logs |
| `Home/End` | (When logs are shown) top/bottom | Top/bottom | Top/bottom |
| `Ctrl+C` | Quit | Quit | Quit |

**Worktrees View:**

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | Switch to Changes view | Return to main changes list |
| `↑/↓` or `j/k` | Navigate worktrees | Move between worktree entries |
| `+` | Create new worktree | Creates new worktree with unique branch name |
| `D` | Delete worktree | Remove non-main, non-processing worktree |
| `M` | Merge to base branch | Merge current worktree branch (conflict-free only) |
| `e` | Open editor | Open editor in worktree directory |
| `Enter` | Open shell | Runs only when `worktree_command` is configured |
| `Ctrl+C` | Quit | Exit application |

*QR code is only available when web monitoring is enabled (`--web` flag). Press any key to close the QR popup.

### TUI Worktree View

The TUI includes a dedicated Worktree View for managing git worktrees directly from the interface.

**Key Features:**

- **View Switching**: Press `Tab` to switch between Changes and Worktrees views
- **Worktree List**: Displays all worktrees with path (basename), branch name, and status
- **Conflict Detection**: Automatically checks for merge conflicts in parallel (background)
- **Branch Merge**: Merge worktree branches to base with `M` key (conflict-free only)
- **Worktree Management**: Create (`+`), delete (`D`), open editor (`e`), open shell (`Enter`)

**Workflow:**

1. **Switch to Worktrees View**: Press `Tab` from Changes view
   - Loads worktree list with conflict detection (runs in parallel)
   - Display format: `<worktree-path> → <branch-name> [STATUS] [⚠conflicts]`

2. **Navigate Worktrees**: Use `↑`/`↓` or `j`/`k` keys
   - Main worktree shown with `[MAIN]` indicator (green)
   - Detached HEAD shown with `[DETACHED]` indicator
   - Conflicts shown with `⚠<count>` badge (red)

3. **Merge Branch**: Press `M` (only enabled when safe)
   - Validates: not main worktree, not detached HEAD, no conflicts
   - Executes: `git merge --no-ff --no-edit <branch>` in base repository
   - On success: Shows success log, refreshes worktree list
   - On failure: Shows error popup with details

4. **Create Worktree**: Press `+`
   - Generates unique branch name: `ws-session-<timestamp>`
   - Creates worktree with new branch (not detached HEAD)
   - Requires `worktree_command` config option

5. **Delete Worktree**: Press `D` (only for non-main, non-processing worktrees)
   - Shows confirmation dialog (`Y` to confirm, `N`/`Esc` to cancel)
   - Removes worktree directory and updates list

6. **Open Editor/Shell**: Press `e` or `Enter`
   - `e`: Opens editor in worktree directory (respects `$EDITOR`)
   - `Enter`: Runs `worktree_command` in worktree (e.g., opens shell)

**Conflict Detection:**

- Runs automatically when switching to Worktrees view
- Checks each non-main, non-detached worktree in parallel using `git merge --no-commit --no-ff`
- Detects conflicts without modifying working tree (uses `git merge --abort`)
- Displays conflict count as `⚠<count>` badge in red
- Updates every 5 seconds (auto-refresh) in background
- Disables `M` key when conflicts detected

**Performance:**

- Parallel conflict checking: Uses async concurrent execution
- Typical performance: 4 worktrees checked in < 1 second
- Non-blocking: Conflict checks run asynchronously, TUI remains responsive
- Fallback: On check failure, assumes no conflict info (safe default)

### Initialize Configuration

Generate a configuration file for your preferred AI agent:

```bash
# Default: Claude Code template
cflx init

# OpenCode template
cflx init --template opencode

# Codex template
cflx init --template codex

# Overwrite existing config
cflx init --force
```

Available templates: `claude` (default), `opencode`, `codex`

### Run Orchestration (Non-Interactive)

Process all pending changes in headless mode:

```bash
cflx run
```

Process specific changes (single or multiple):

```bash
# Single change
cflx run --change add-feature-x

# Multiple changes (comma-separated)
cflx run --change add-feature-x,fix-bug-y,refactor-z
```

Custom configuration file:

```bash
cflx run --config /path/to/config.jsonc
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
1. `.cflx.jsonc` (project root)
2. `~/.config/cflx/config.jsonc` (global)
3. Custom path via `--config` option

**Example configuration (Claude Code):**

```jsonc
{
  // Command to analyze dependencies and select next change
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // Command to apply a change (supports {change_id} and {prompt} placeholders)
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",

  // Command to run acceptance tests after apply (supports {change_id} and {prompt} placeholders)
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:accept {change_id} {prompt}'",

  // Command to archive a completed change (supports {change_id} and {prompt} placeholders)
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  // Command to resolve merge conflicts (supports {prompt} placeholder)
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // System prompt for apply command (injected into {prompt} placeholder)
  "apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。",

  // System prompt for acceptance command (injected into {prompt} placeholder)
  "acceptance_prompt": "",

  // Controls how the acceptance {prompt} is constructed
  // - "full": include hardcoded acceptance system prompt + diff/history context (default)
  // - "context_only": only include change metadata + diff/history context
  // Use "context_only" when your acceptance_command uses a command template with fixed instructions
  "acceptance_prompt_mode": "full",

  // Maximum number of acceptance CONTINUE retries before treating as FAIL (default: 10)
  "acceptance_max_continues": 10,

  // System prompt for archive command (injected into {prompt} placeholder)
  "archive_prompt": "",

  // Command to create a proposal worktree from TUI (+ key)
  // Supports {workspace_dir} and {repo_root} placeholders
  "worktree_command": "claude --dangerously-skip-permissions --verbose -p '/openspec:proposal --worktree {workspace_dir}'",

  // Lifecycle hooks (optional)
  "hooks": {
    // "pre_apply": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Completed {change_id}'"
  }
}
```

**Logging configuration:**

```jsonc
{
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  }
}
```

- `suppress_repetitive_debug`: suppress repeated debug logs when state is unchanged (default: true)
- `summary_interval_secs`: emit summary logs every N seconds, set to 0 to disable (default: 60)

**Placeholders:**

| Placeholder | Description | Used in |
|-------------|-------------|---------|
| `{change_id}` | The change ID being processed | apply_command, acceptance_command, archive_command |
| `{prompt}` | System prompt for agent commands | apply_command, acceptance_command, archive_command, resolve_command, analyze_command |
| `{workspace_dir}` | New worktree path for proposals | worktree_command |
| `{repo_root}` | Repository root path | worktree_command |

**System Prompts:**

| Config Key | Description | Default |
|------------|-------------|---------|
| `apply_prompt` | Prompt injected into apply_command's `{prompt}` | (includes path context) |
| `acceptance_prompt` | Prompt injected into acceptance_command's `{prompt}` | (empty) |
| `archive_prompt` | Prompt injected into archive_command's `{prompt}` | (empty) |

**Quick start:**

```bash
# Generate configuration with init command
cflx init

# Or copy the example configuration
cp .cflx.jsonc.example .cflx.jsonc

# Edit to customize settings
vim .cflx.jsonc

# Run with the configuration
cflx
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
cflx

# Use a specific version via npx
export OPENSPEC_CMD="npx @fission-ai/openspec@1.2.3"
cflx
```

### Command-line Options

```
Usage: cflx [OPTIONS] [COMMAND]

Commands:
  run              Run the OpenSpec change orchestration loop (non-interactive)
  tui              Launch the interactive TUI dashboard
  init             Initialize a new configuration file
  check-conflicts  Check for conflicts between spec delta files across changes
  server           Start the multi-project server daemon

Options:
  -c, --config <PATH>          Path to custom configuration file (JSONC format)
  --web                        Enable web monitoring server for remote status viewing
  --web-port <PORT>            Port for web monitoring server (default: 0 = auto-assign by OS)
  --web-bind <ADDR>            Bind address for web monitoring server (default: 127.0.0.1)
  --server <URL>               Connect TUI to a remote Conflux server (e.g., http://host:9876)
  --server-token <TOKEN>       Bearer token for remote server authentication
  --server-token-env <VAR>     Environment variable holding the bearer token
  -h, --help                   Print help
  -V, --version                Print version
```

**Run subcommand options:**
```
Options:
  --change <ID,...>         Process only specified changes (comma-separated)
  -c, --config <PATH>       Custom configuration file path (JSONC)
  --parallel                Enable parallel execution mode
  --max-concurrent <N>      Maximum concurrent workspaces (default: 3)
  --vcs <BACKEND>           VCS backend: auto or git (default: auto)
  --no-resume               Disable workspace resume (always create new workspaces)
  --dry-run                 Preview parallelization groups without executing
  --max-iterations <N>      Maximum number of orchestration loop iterations (0 = no limit)
  --web                     Enable web monitoring server
  --web-port <PORT>         Web server port (default: 0 = auto-assign by OS)
  --web-bind <ADDR>         Web server bind address (default: 127.0.0.1)
```

**TUI options:**

The TUI (default mode, `cflx` or `cflx tui`) also supports web monitoring options:

```bash
# TUI with web monitoring
cflx --web

# Custom port and bind address
cflx --web --web-port 9000 --web-bind 0.0.0.0
```

### Parallel Execution

The orchestrator supports parallel execution of independent changes using Git worktrees.

**VCS Backend Selection:**

| Backend | Description | Requirements |
|---------|-------------|--------------|
| `auto` | Auto-detect Git repository | Git repository with clean working directory |
| `git` | Use Git worktrees | Git repository with clean working directory |

**Usage:**

```bash
# Auto-detect VCS backend (default)
cflx run --parallel

# Force Git worktrees
cflx run --parallel --vcs git

# Preview parallelization groups without executing
cflx run --parallel --dry-run

# Limit concurrent workspaces
cflx run --parallel --max-concurrent 5
```

**Configuration:**

You can also set the VCS backend in your configuration file:

```jsonc
{
  // VCS backend for parallel execution: "auto" or "git"
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

**Workspace Resume:**

By default, the orchestrator automatically detects and reuses existing workspaces from interrupted runs. This allows you to resume work from where you left off without losing progress.

- When a workspace is found for a change ID, it is reused instead of creating a new one
- If multiple workspaces exist for the same change, the newest one is used and older ones are cleaned up
- Use `--no-resume` to disable this behavior and always create fresh workspaces

```bash
# Resume from existing workspaces (default behavior)
cflx run --parallel

# Always create new workspaces (discard any existing work)
cflx run --parallel --no-resume
```

**Workspace State Detection (Idempotent Resume):**

The orchestrator detects the current state of each workspace to ensure idempotent execution. When resuming, workspaces are classified into one of five states:

| State | Description | Action Taken |
|-------|-------------|--------------|
| **Created** | New workspace, no commits yet | Start apply from beginning |
| **Applying** | WIP commits exist, apply in progress | Resume apply from next iteration |
| **Applied** | Apply complete (`Apply: <change_id>` commit exists) | Skip apply, run archive only |
| **Archived** | Archive complete (`Archive: <change_id>` commit exists) | Skip apply/archive, run merge only |
| **Merged** | Already merged to main branch | Skip all operations, cleanup workspace |

This state detection ensures that:
- Running the orchestrator multiple times on the same workspace is safe and produces the same result (idempotency)
- Manually archived or merged changes are detected and handled correctly
- Interrupted operations resume from the correct step
- No duplicate work is performed

**State Detection Examples:**

```bash
# Interrupted during apply - resumes from where it left off
$ cflx run --parallel
# Workspace state: Applying (iteration 3/5)
# Action: Resume apply from iteration 4

# Manually archived a change - skips apply/archive
$ cflx run --parallel
# Workspace state: Archived
# Action: Skip apply/archive, merge to main only

# Already merged to main - cleanup only
$ cflx run --parallel
# Workspace state: Merged
# Action: Skip all operations, cleanup workspace
```

### Command Execution Queue

The orchestrator includes a command execution queue that prevents resource conflicts and handles transient errors when running multiple AI agent commands in parallel.

**Features:**

1. **Staggered Start**: Commands are started with a configurable delay to prevent simultaneous resource access
2. **Automatic Retry**: Commands that fail due to transient errors (module resolution, network issues, etc.) are automatically retried

**Configuration:**

```jsonc
{
  // Delay between command executions (milliseconds)
  // Default: 2000 (2 seconds)
  "command_queue_stagger_delay_ms": 2000,

  // Maximum number of retries for failed commands
  // Default: 2
  "command_queue_max_retries": 2,

  // Delay between retries (milliseconds)
  // Default: 5000 (5 seconds)
  "command_queue_retry_delay_ms": 5000,

  // Retry if execution duration is under this threshold (seconds)
  // Short-running failures often indicate environment/startup issues
  // Default: 5
  "command_queue_retry_if_duration_under_secs": 5,

  // Error patterns that trigger automatic retry (regex)
  // Default: module resolution, registry, and lock errors
  "command_queue_retry_patterns": [
    "Cannot find module",
    "ResolveMessage:",
    "ENOTFOUND registry\\.npmjs\\.org",
    "ETIMEDOUT.*registry",
    "EBADF.*lock",
    "Lock acquisition failed"
  ]
}
```

**How It Works:**

- **Staggered Start**: Each command waits for a minimum delay since the last command started, preventing simultaneous access to shared resources (e.g., `~/.cache/opencode/node_modules`)
- **Retry Logic**: Commands are retried if they:
  - Match a configured error pattern (e.g., "Cannot find module"), OR
  - Exit quickly (< 5 seconds by default), indicating a startup/environment issue
- **No Retry**: Commands that run for a long time (> 5 seconds) and don't match error patterns are not retried, as they likely failed due to logical errors

**Example - Preventing Module Resolution Conflicts:**

```bash
# Without queue: Multiple commands start simultaneously
# → Conflict: All try to update node_modules at once
# → Result: "Cannot find module" errors

# With queue (default): Commands start 2 seconds apart
# → First command updates node_modules
# → Subsequent commands use stable environment
# → Result: No conflicts
```

**Example - Handling Transient Network Errors:**

```bash
# Error: ETIMEDOUT registry.npmjs.org
# → Matches retry pattern
# → Automatically retried after 5 seconds
# → Usually succeeds on retry
```

### Web Monitoring

The orchestrator supports an optional HTTP server for remote monitoring of orchestration progress via web browser.

**Usage:**

```bash
# Enable web monitoring with TUI (OS auto-assigns an available port)
cflx --web

# Custom port and bind address
cflx --web --web-port 9000 --web-bind 0.0.0.0

# With headless run mode
cflx run --web
```

When using the default port (0), the OS automatically assigns an available port.
The actual bound address is logged when the server starts.

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
| `/api/changes/{id}/approve` | POST | Approve a change |
| `/api/changes/{id}/unapprove` | POST | Unapprove a change |

For complete API specifications, see the [OpenAPI documentation](docs/openapi.yaml).

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
| "Address already in use" | Use `--web-port 0` (default) to let the OS auto-assign an available port, or specify a specific unused port |
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

**Check-conflicts subcommand options:**
```
Options:
  -j, --json  Output results in JSON format
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
- Check your configuration file: `.cflx.jsonc`

### "All changes failed"

- Check logs for specific errors
- Try processing a single change: `--change <id>`

## Installation

```bash
cargo install --path .
```

This will build and install the orchestrator to your Cargo bin directory (typically `~/.cargo/bin`).

## Documentation

| Document | Description |
|----------|-------------|
| [Usage Examples](docs/guides/USAGE.md) | Quick start and usage examples |
| [Development Guide](docs/guides/DEVELOPMENT.md) | Build instructions and project structure |
| [Release Guide](docs/guides/RELEASE.md) | How to create releases |
| [API Specification](docs/openapi.yaml) | OpenAPI spec for web monitoring |

Internal documentation (parallel execution audit) is available in `docs/audit/`.

## Project Structure

```
src/
  main.rs                   # Entry point, CLI dispatching
  cli.rs                    # CLI argument parsing (clap)
  error.rs                  # Error types (thiserror)
  openspec.rs               # OpenSpec CLI wrapper
  orchestrator.rs           # Main orchestration loop
  progress.rs               # Progress display (indicatif)
  hooks.rs                  # Lifecycle hook execution
  task_parser.rs            # Native tasks.md parser
  templates.rs              # Configuration templates
  acceptance.rs             # Acceptance test output parsing
  ai_command_runner.rs      # Common AI command runner (unified stagger state)
  analyzer.rs               # Change dependency analyzer
  command_queue.rs          # Command queue with stagger and retry
  events.rs                 # Unified event system
  history.rs                # Apply/archive/resolve history
  error_history.rs          # Error history tracking
  merge_stall_monitor.rs    # Merge stall detection monitor
  parallel_run_service.rs   # Parallel execution service
  permission.rs             # Permission auto-reject detection
  process_manager.rs        # Cross-platform child process management
  serial_run_service.rs     # Serial execution service
  spec_delta.rs             # Spec delta parsing and conflict detection
  spec_test_annotations.rs  # Spec test annotation support
  stall.rs                  # Stall detection utilities
  worktree_ops.rs           # Common worktree operations (TUI and Web)

  agent/                    # AI agent command execution
    mod.rs                  # Agent runner module
    runner.rs               # Agent runner implementation
    output.rs               # Output line types
    prompt.rs               # Prompt building functions
    history_ops.rs          # History management operations
    tests.rs                # Agent module tests

  execution/                # Shared execution logic
    apply.rs                # Apply operation logic
    archive.rs              # Archive operation logic
    state.rs                # Workspace state detection
    types.rs                # Common type definitions

  orchestration/            # Shared orchestration logic
    mod.rs                  # Common CLI/TUI orchestration
    acceptance.rs           # Shared acceptance operations
    apply.rs                # Shared apply operations
    archive.rs              # Shared archive operations
    hooks.rs                # Hook context helpers
    output.rs               # Output handler trait
    selection.rs            # Shared change selection logic
    state.rs                # Shared state management

  config/                   # Configuration
    defaults.rs             # Default values
    expand.rs               # Environment variable expansion
    jsonc.rs                # JSONC parser

  vcs/                      # Version Control abstraction
    commands.rs             # Common VCS interface
    git/                    # Git backend
      commands/             # Git command implementations
        basic.rs            # Basic git operations
        commit.rs           # Commit operations
        merge.rs            # Merge operations
        worktree.rs         # Worktree management

  parallel/                 # Parallel execution
    executor.rs             # Parallel change executor
    events.rs               # Progress reporting events
    conflict.rs             # Conflict detection/resolution
    cleanup.rs              # Workspace cleanup
    dynamic_queue.rs        # Dynamic queue for runtime change additions
    merge.rs                # Merge operations for parallel execution
    output_bridge.rs        # Bridge between OutputHandler and ParallelEvent
    types.rs                # Common types for parallel execution
    workspace.rs            # Workspace creation and management

  remote/                   # Remote server client (TUI)
    mod.rs                  # HTTP and WebSocket client module
    client.rs               # HTTP client for remote server
    mapper.rs               # Remote type to local type mapping
    types.rs                # Remote communication type definitions
    ws.rs                   # WebSocket client for real-time updates

  server/                   # Server daemon (multi-project management)
    mod.rs                  # Server daemon module
    api.rs                  # REST API handlers
    registry.rs             # Project registry with persistence
    runner.rs               # Server-side project runner

  web/                      # Web monitoring
    mod.rs                  # Web monitoring module
    api.rs                  # REST API handlers
    state.rs                # Web monitoring state management
    url.rs                  # URL conversion utilities
    websocket.rs            # WebSocket handler for real-time updates

  tui/                      # Terminal User Interface
    mod.rs                  # TUI module
    render.rs               # Terminal rendering
    runner.rs               # TUI main loop
    state.rs                # TUI state management
    types.rs                # TUI type definitions
    type_impls.rs           # TUI type implementations
    events.rs               # TUI event handling
    command_handlers.rs     # TuiCommand handlers
    key_handlers.rs         # Key event handlers
    orchestrator.rs         # TUI orchestrator execution logic
    terminal.rs             # Terminal helper functions
    queue.rs                # Dynamic queue for runtime change additions
    worktrees.rs            # Worktree view management
    log_deduplicator.rs     # Log deduplication
    qr.rs                   # QR code generation for Web UI URL

tests/
  e2e_tests.rs              # End-to-end tests
```

## Development

See [Development Guide](docs/guides/DEVELOPMENT.md) for build instructions, testing, and project structure.

### Git Hooks

This project uses [prek](https://prek.j178.dev/) for managing Git hooks (a Rust-based alternative to pre-commit).

**Migration from pre-commit:**

If you were previously using pre-commit, uninstall it first:

```bash
pre-commit uninstall
```

**Setup:**

```bash
# Install prek
brew install prek

# Install hooks
prek install
```

**Usage:**

```bash
# Run all hooks on all files
prek run --all-files

# Run specific hooks
prek run rustfmt clippy

# List available hooks
prek list
```

The configuration is defined in `.pre-commit-config.yaml` (prek is fully compatible with pre-commit configuration format). The `prek run --all-files` command also auto-runs `make openapi` and stages `docs/openapi.yaml`.

## Future Enhancements

- [ ] State persistence for recovery and resumption
- [x] Parallel execution for independent changes (using Git worktrees)
- [ ] Slack/Discord notifications
- [ ] Maximum iteration limit (prevent infinite loops)
- [ ] Manual priority override
- [ ] Enhanced dry-run with execution plan
- [ ] Web UI for monitoring

## License

MIT

## Contributing

Contributions welcome! Please open an issue or pull request.
