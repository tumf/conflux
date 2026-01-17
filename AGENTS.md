<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# AGENTS.md - Conflux

This document provides essential information for AI coding agents working on this Rust codebase.

## Project Overview

Conflux automates the OpenSpec change workflow (list -> dependency analysis -> apply -> archive). It orchestrates `openspec` and AI coding agent tools to process changes autonomously.

## Build Commands

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Clean
cargo clean
```

## Lint Commands

```bash
# Format code (check)
cargo fmt --check

# Format code (apply)
cargo fmt

# Clippy lints
cargo clippy

# Clippy with warnings as errors
cargo clippy -- -D warnings
```

## Test Commands

```bash
# Run all tests
cargo test

# Run a single test by name
cargo test test_single_change_flow_dry_run

# Run tests matching a pattern
cargo test test_state

# Run tests with output
cargo test -- --nocapture

# Run tests in a specific file
cargo test --test e2e_tests

# Run tests in a specific module
cargo test openspec::tests

# Run with verbose output
cargo test -- --show-output
```

## Run Commands

```bash
# Run with debug logging
RUST_LOG=debug cargo run -- run --dry-run

# Run in release mode
cargo run --release -- run
```

## Project Structure

```
src/
  main.rs               # Entry point, CLI dispatching
  cli.rs                # CLI argument parsing (clap)
  error.rs              # Error types (thiserror)
  openspec.rs           # OpenSpec CLI wrapper
  orchestrator.rs       # Main orchestration loop
  progress.rs           # Progress display (indicatif)

  # Core modules
  agent.rs              # AI agent command execution
  analyzer.rs           # Change dependency analyzer for parallel execution
  approval.rs           # Change approval management
  command_queue.rs      # Command execution queue with stagger and retry
  history.rs            # Apply, archive, and resolve context history management
  hooks.rs              # Lifecycle hook execution
  parallel_run_service.rs # Parallel execution service
  task_parser.rs        # Native task.md parser
  templates.rs          # Configuration templates

  # Execution (shared logic between serial and parallel modes)
  execution/
    mod.rs              # Execution module root
    apply.rs            # Common apply operation logic
    archive.rs          # Common archive operation logic
    state.rs            # Workspace state detection for idempotent resume
    types.rs            # Common type definitions

  # Configuration
  config/
    mod.rs              # Configuration module root
    defaults.rs         # Default configuration values
    expand.rs           # Environment variable expansion
    jsonc.rs            # JSONC parser (JSON with comments)

  # VCS (Version Control System) abstraction
  vcs/
    mod.rs              # VCS module root, backend trait abstraction
    commands.rs         # Common VCS command interface

    git/
      mod.rs            # Git backend implementation
      commands.rs       # Git command wrappers

    jj/
      mod.rs            # jj backend implementation
      commands.rs       # jj command wrappers

  # Parallel execution
  parallel/
    mod.rs              # Parallel execution module root
    executor.rs         # Parallel change executor
    types.rs            # Shared types for parallel execution
    events.rs           # Event types for progress reporting
    conflict.rs         # Conflict detection and resolution
    cleanup.rs          # Workspace cleanup utilities

  # TUI (Terminal User Interface)
  tui/
    mod.rs              # TUI module root
    events.rs           # TUI event types
    orchestrator.rs     # TUI orchestration integration
    parallel_event_bridge.rs # Bridge between parallel executor and TUI
    queue.rs            # Event queue management
    render.rs           # Terminal rendering
    runner.rs           # TUI runner/main loop
    types.rs            # TUI-specific types
    utils.rs            # TUI utility functions

    state/
      mod.rs            # TUI state module root
      change.rs         # Change state management
      events.rs         # State event handling
      logs.rs           # Log state management
      modes.rs          # TUI mode state

tests/
  e2e_tests.rs           # End-to-end tests with mock scripts
  ralph_compatibility.rs # Ralph plugin compatibility tests
```

## TUI Features

### Worktree View

The TUI includes a dedicated Worktree View for managing git worktrees with integrated merge functionality.

**Key Features**:
- **View Switching**: Press `Tab` to switch between Changes and Worktrees views
- **Worktree List**: Displays all worktrees with path (basename), branch name, and status
- **Conflict Detection**: Automatically checks for merge conflicts in parallel (background)
- **Branch Merge**: Merge worktree branches to base with `M` key (conflict-free only)
- **Worktree Management**: Create (`+`), delete (`D`), open editor (`e`), open shell (`Enter`)

**Workflow**:

1. **Switch to Worktrees View**: Press `Tab` from Changes view
   - Loads worktree list with conflict detection (runs in parallel)
   - Displays: `<worktree-path> → <branch-name> [STATUS] [⚠conflicts]`

2. **Navigate Worktrees**: Use `↑`/`↓` or `j`/`k` keys
   - Main worktree shown with `[MAIN]` indicator (green)
   - Detached HEAD shown with `[DETACHED]` indicator
   - Conflicts shown with `⚠<count>` badge (red)

3. **Merge Branch**: Press `M` (only enabled when safe)
   - Validates: not main worktree, not detached HEAD, no conflicts
   - Executes: `git merge --no-ff --no-edit <branch>` in worktree
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

**Conflict Detection**:

- Runs automatically when switching to Worktrees view
- Checks each non-main, non-detached worktree in parallel using `git merge --no-commit --no-ff`
- Detects conflicts without modifying working tree (uses `git merge --abort`)
- Displays conflict count as `⚠<count>` badge in red
- Updates every 5 seconds (auto-refresh) in background
- Disables `M` key when conflicts detected

**Performance**:

- Parallel conflict checking: Uses `tokio::task::JoinSet` for concurrent execution
- Typical performance: 4 worktrees checked in < 1 second
- Non-blocking: Conflict checks run asynchronously, TUI remains responsive
- Fallback: On check failure, assumes no conflict info (safe default)

**Key Bindings** (Worktrees View):

| Key | Action | Condition |
|-----|--------|-----------|
| `Tab` | Switch to Changes view | Always |
| `↑`/`↓`, `j`/`k` | Navigate worktrees | Always |
| `+` | Create new worktree | `worktree_command` configured |
| `D` | Delete worktree | Not main, not processing |
| `M` | Merge to base branch | Not main, not detached, no conflicts, has branch |
| `e` | Open editor | Always |
| `Enter` | Open shell | `worktree_command` configured |
| `q` | Quit | Always |

**Troubleshooting**:

**Issue**: Merge conflicts not detected
- **Cause**: Conflict check failed or timed out
- **Solution**: Check git repository health, ensure worktree is accessible

**Issue**: Cannot merge (conflicts detected)
- **Cause**: Branch has merge conflicts with base
- **Solution**: Resolve conflicts manually in worktree, then retry merge

**Issue**: Slow worktree view switching
- **Cause**: Many worktrees (> 10) with slow conflict checks
- **Solution**: Normal for large number of worktrees; conflict checks run in parallel

**Issue**: `M` key not showing in footer
- **Cause**: Worktree has conflicts, is detached HEAD, or is main worktree
- **Solution**: Check worktree status; only clean, non-main, branched worktrees can merge

## Code Style Guidelines

### Imports

- Group imports in three sections: `std` → external crates → `crate::`
- Use `crate::` prefix for all internal module imports
- Use specific imports, avoid glob imports (`use foo::*`)
- Sort imports alphabetically within each group

```rust
// std imports
use std::path::PathBuf;
use std::process::Command;

// External crates
use regex::Regex;
use tracing::{debug, info};

// Internal modules
use crate::error::{OrchestratorError, Result};
use crate::openspec;
```

### Error Handling

- Use `thiserror` for custom error type definitions
- Define custom `Result<T>` type alias: `pub type Result<T> = std::result::Result<T, OrchestratorError>;`
- Use `?` operator for error propagation
- Use `map_err` for adding context to errors
- Use `#[from]` attribute for automatic error conversions

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Agent command failed: {0}")]
    AgentCommand(String),

    #[error("VCS error: {0}")]
    Vcs(#[from] VcsError),
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;

// Usage
fn example() -> Result<String> {
    let data = std::fs::read_to_string("file.txt")?;  // Auto-converts io::Error
    Ok(data)
}
```

### Naming Conventions

- **Types/Structs/Enums**: `PascalCase` (e.g., `OrchestratorState`, `AgentRunner`, `VcsBackend`)
- **Functions/Methods**: `snake_case` (e.g., `list_changes`, `run_command`, `detect_workspace_state`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `STATE_FILE`, `DEFAULT_MAX_RETRIES`, `APPLY_SYSTEM_PROMPT`)
- **Modules**: `snake_case` (e.g., `openspec.rs`, `orchestrator.rs`, `command_queue.rs`)
- **Lifetimes**: Single lowercase letter or descriptive (e.g., `'a`, `'static`)

### Struct Definitions

- Use `#[derive(...)]` for common traits (Debug, Clone, Serialize, Deserialize)
- Place `#[allow(dead_code)]` on unused fields only if intentional (document why)
- Use doc comments (`///`) for all public APIs
- Use module-level doc comments (`//!`) at the top of files

```rust
//! Agent runner module for executing configurable agent commands.

use serde::{Deserialize, Serialize};

/// Manages agent process execution based on configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    /// Unique identifier for the change
    pub id: String,
    /// Number of completed tasks
    pub completed_tasks: u32,
    /// Total number of tasks
    pub total_tasks: u32,
    /// Last modification timestamp (ISO 8601)
    #[allow(dead_code)] // Kept for future audit features
    pub last_modified: String,
}
```

### Async Code

- Use `tokio` runtime with `#[tokio::main]` or `#[tokio::test]`
- Use `async/await` for all async functions
- Prefer `tokio::process::Command` for async process execution (not `std::process::Command`)
- Use `tokio::time::sleep` for delays (not `std::thread::sleep`)
- Use `tokio::fs` for async file operations when in async context

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let output = tokio::process::Command::new("ls")
        .arg("-la")
        .output()
        .await?;
    Ok(())
}
```

### Logging

- Use `tracing` crate for all logging (not `println!` or `eprintln!`)
- Log levels: `error!`, `warn!`, `info!`, `debug!`, `trace!`
- Initialize with `tracing_subscriber` in `main.rs`
- Use structured logging with key-value pairs when helpful

```rust
use tracing::{debug, error, info, warn};

info!("Starting orchestrator");
debug!(status = ?exit_status, "Agent command exited");
warn!(change_id = %id, "Archive verification failed");
error!(error = %e, "Failed to execute command");
```

### Testing

- Unit tests: Place in `#[cfg(test)]` modules within source files
- Integration tests: Place in `tests/` directory as separate files
- Use `tempfile` crate for temporary test files/directories
- Use `#[tokio::test]` for async tests
- Mock external commands with shell scripts for E2E tests
- Name test functions descriptively: `test_<what>_<expected_behavior>`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_task_counts() {
        let result = parse_tasks("- [x] Done\n- [ ] Todo");
        assert_eq!(result.completed, 1);
        assert_eq!(result.total, 2);
    }

    #[tokio::test]
    async fn test_async_operation() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Comments

- Use `//` for line comments, `/* */` for block comments
- Write comments that explain **why**, not **what** (code should be self-documenting)
- Use `TODO:` comments sparingly with context
- Use doc comments (`///` or `//!`) for public APIs

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| tokio | Async runtime |
| clap | CLI argument parsing |
| serde/serde_json | Serialization |
| anyhow | Error handling (unused, prefer thiserror) |
| thiserror | Error type definitions |
| tracing | Logging |
| indicatif | Progress bars |
| regex | Pattern matching |
| chrono | Date/time handling |
| tempfile | Test fixtures |
| async-trait | Async trait definitions |
| nix (Unix) | Process group management |
| windows (Windows) | Job object management |

## Process Management

The orchestrator uses platform-specific process management to ensure reliable cleanup of child processes:

### Unix (macOS/Linux)

- **Process Groups**: Child processes are spawned in their own process group using `setpgid(0, 0)`
- **Cleanup**: On termination, `killpg()` sends SIGTERM to the entire process group
- **Implementation**: See `src/process_manager.rs` and `src/agent.rs`

### Windows

- **Job Objects**: Child processes are assigned to a job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
- **Cleanup**: When the job handle is closed, Windows automatically terminates all processes in the job
- **Implementation**: See `src/process_manager.rs`

### Signal Handling (Run Mode)

The `run` mode includes signal handlers for graceful shutdown:
- **SIGINT (Ctrl+C)**: Handled on all platforms
- **SIGTERM**: Handled on Unix platforms
- **Behavior**: Cancels running operations and waits for child processes to terminate

### Troubleshooting

**Issue**: Child processes remain after orchestrator exit
- **Unix**: Check if process group was created: `ps -o pid,pgid -p <pid>`
- **Windows**: Verify job object assignment (use Process Explorer)
- **Solution**: Ensure `ManagedChild` is properly created and terminated

**Issue**: Timeout waiting for process cleanup
- **TUI Mode**: Default timeout is 5 seconds
- **Solution**: If processes take longer, check for hung child processes or increase timeout in `src/tui/runner.rs`

## Command Execution Queue

The orchestrator uses a command execution queue to prevent resource conflicts and handle transient errors when running multiple AI agent commands.

### Overview

**Module**: `src/command_queue.rs`

The command queue provides two key features:
1. **Staggered Start**: Introduces a configurable delay between command executions to prevent simultaneous resource access
2. **Automatic Retry**: Retries commands that fail due to transient errors (module resolution, network issues, etc.)

### Architecture

```
AgentRunner
    ↓
CommandQueue (stagger + retry)
    ↓
tokio::process::Command (spawn)
```

### Staggered Start

Commands are started with a minimum delay to prevent resource conflicts:

```rust
async fn execute_with_stagger(&self, command_fn: F) -> Result<Child> {
    // Wait for minimum delay since last command
    let mut last = self.last_execution.lock().await;
    if let Some(last_time) = *last {
        let elapsed = last_time.elapsed();
        if elapsed < self.config.stagger_delay_ms {
            tokio::time::sleep(self.config.stagger_delay_ms - elapsed).await;
        }
    }
    *last = Some(Instant::now());

    // Spawn command
    command_fn().spawn()
}
```

**Usage**: Applied to all streaming commands (apply, archive, resolve)

### Automatic Retry

Commands are retried if they fail due to transient errors:

**Retry Decision Logic** (OR condition):
1. **Error Pattern Match**: stderr matches configured regex patterns (e.g., "Cannot find module")
2. **Short Execution**: Command exits in < 5 seconds (default), indicating startup/environment issues

**No Retry** for:
- Successful commands (exit code 0)
- Long-running failures (> 5 seconds) without pattern match (likely logical errors)
- Max retry count exceeded

**Implementation**:

```rust
fn should_retry(&self, attempt: u32, duration: Duration, stderr: &str, exit_code: i32) -> bool {
    if attempt >= self.config.max_retries || exit_code == 0 {
        return false;
    }

    // Retry if: error pattern match OR short execution
    let matches_pattern = self.is_retryable_error(stderr);
    let is_short = duration < Duration::from_secs(self.config.retry_if_duration_under_secs);

    matches_pattern || is_short
}
```

### Configuration

| Config Key | Default | Description |
|------------|---------|-------------|
| `command_queue_stagger_delay_ms` | 2000 | Delay between command starts (ms) |
| `command_queue_max_retries` | 2 | Maximum retry attempts |
| `command_queue_retry_delay_ms` | 5000 | Delay between retries (ms) |
| `command_queue_retry_if_duration_under_secs` | 5 | Retry threshold for short runs (seconds) |
| `command_queue_retry_patterns` | See below | Regex patterns for retryable errors |

**Default Retry Patterns**:
- `Cannot find module` - Module resolution failures
- `ResolveMessage:` - Module resolve errors
- `ENOTFOUND registry\.npmjs\.org` - NPM registry unavailable
- `ETIMEDOUT.*registry` - Registry timeout
- `EBADF.*lock` - File lock errors
- `Lock acquisition failed` - Lock contention

### Integration Points

**AgentRunner** (`src/agent.rs`):
- `execute_shell_command_streaming()` - Uses `execute_with_stagger()` for apply/archive
- `execute_shell_command_streaming_in_dir()` - Uses `execute_with_stagger()` for resolve
- `build_command()` - Extracts command building logic for queue integration
- `build_command_in_dir()` - Command builder for directory-scoped commands

### Troubleshooting

**Issue**: Commands still conflict despite staggered start
- **Cause**: Stagger delay too short for initialization
- **Solution**: Increase `command_queue_stagger_delay_ms` (e.g., 3000-5000ms)

**Issue**: Commands retry too aggressively
- **Cause**: Retry threshold too high or patterns too broad
- **Solution**: Reduce `command_queue_retry_if_duration_under_secs` or narrow retry patterns

**Issue**: Transient errors not retried
- **Cause**: Error pattern not in retry list
- **Solution**: Add custom pattern to `command_queue_retry_patterns`

## Configuration Files

- Project config: `.cflx.jsonc` (JSONC format with comments)
- Global config: `~/.cflx.jsonc`

## Retry Context History

The orchestrator tracks retry history for apply, archive, and resolve operations to help AI agents learn from previous attempts.

### Apply History

- **Module**: `src/history.rs` - `ApplyHistory`
- **Purpose**: Tracks apply command attempts per change
- **Recording**: After each apply attempt (success or failure)
- **Context Format**: XML-like tags with attempt number, status, duration, error, and exit code
- **Clearing**: When change is successfully archived
- **Example**:
  ```xml
  <last_apply attempt="1">
  status: failed
  duration: 45s
  error: Type error in auth.rs:42
  exit_code: 1
  </last_apply>
  ```

### Archive History

- **Module**: `src/history.rs` - `ArchiveHistory`
- **Purpose**: Tracks archive command attempts per change
- **Recording**: After each archive attempt and verification
- **Context Format**: XML-like tags with attempt number, status, duration, verification result, error, and exit code
- **Clearing**: When change is successfully archived
- **Example**:
  ```xml
  <last_archive attempt="1">
  status: failed
  duration: 5s
  verification_result: Change still exists at openspec/changes/my-change
  error: Archive command succeeded but verification failed
  exit_code: 0
  </last_archive>
  ```

### Resolve Context

- **Module**: `src/history.rs` - `ResolveContext`
- **Purpose**: Tracks resolve attempts within a single retry session (not persisted across changes)
- **Recording**: After each resolve attempt and verification
- **Context Format**: XML-like tags with attempt count, previous attempt details, and continuation reason
- **Clearing**: Automatically when resolve session completes (function scope)
- **Used By**:
  - `src/parallel/conflict.rs` - `resolve_conflicts_with_retry()`
  - `src/parallel/conflict.rs` - `resolve_merges_with_retry()`
- **Example**:
  ```xml
  <resolve_context>
  This is attempt 2 of 3 for conflict resolution.

  Previous attempt (1):
  - Command exit: success (code: 0)
  - Verification: failed
  - Reason: Conflicts still present after resolution attempt: src/main.rs, src/lib.rs
  - Duration: 45s

  Continue resolving the conflicts. The previous attempt did not fully resolve all conflicts.
  </resolve_context>
  ```

### Implementation Notes

- All history is kept in memory (not persisted to disk)
- History is passed to AI agents via prompt injection
- Archive and apply history are per-change and cleared on successful archive
- Resolve context is per-session and exists only during the retry loop
- Recording failures are logged as warnings but don't stop execution

## Common Patterns

### Command Execution

```rust
let output = Command::new(path)
    .arg("list")
    .output()
    .map_err(|e| OrchestratorError::OpenSpecCommand(format!("Failed: {}", e)))?;

if !output.status.success() {
    return Err(OrchestratorError::OpenSpecCommand(stderr));
}
```

### State Persistence

```rust
// Load
let state = OrchestratorState::load()?.unwrap_or_else(OrchestratorState::new);

// Save
self.state.touch();  // Update timestamp
self.state.save()?;
```

## Workspace State Detection (Parallel Mode)

### Overview

The parallel execution mode uses workspace state detection to enable idempotent resume operations. When resuming a workspace, the orchestrator detects the current state and determines the appropriate action.

### Workspace States

| State | Detection Criteria | Action Taken |
|-------|-------------------|--------------|
| **Created** | No commits, fresh workspace | Start apply from beginning |
| **Applying** | WIP commits exist: `WIP(apply): <change_id> (iteration N/M)` | Resume apply from next iteration |
| **Applied** | Apply commit exists: `Apply: <change_id>` | Skip apply, run archive only |
| **Archived** | Archive commit exists: `Archive: <change_id>` (not in base branch) | Skip apply/archive, run merge only |
| **Merged** | Archive commit found in base branch | Skip all operations, cleanup workspace |

### State Detection Module

**Location**: `src/execution/state.rs`

**Key Functions**:

- `detect_workspace_state(change_id, repo_root, base_branch)` - Main entry point, returns `WorkspaceState`
- `is_merged_to_base(change_id, repo_root, base_branch)` - Check if archive commit is in base branch
- `has_apply_commit(change_id, repo_root)` - Check for apply completion
- `get_latest_wip_snapshot(change_id, repo_root)` - Get highest WIP iteration number
- `is_archive_commit_complete(change_id, repo_root)` - Verify archive commit and clean working tree

### Integration

**Location**: `src/parallel/mod.rs` - `execute_group()` function

When resuming a workspace:

```rust
// Get the original/base branch name
let original_branch = workspace_manager.original_branch()
    .ok_or_else(|| OrchestratorError::GitCommand("Original branch not initialized".to_string()))?;

let workspace_state = detect_workspace_state(change_id, &workspace.path, &original_branch).await?;

match workspace_state {
    WorkspaceState::Merged => {
        // Skip all operations, cleanup workspace
        cleanup_workspace(&workspace.name).await?;
    }
    WorkspaceState::Archived => {
        // Skip apply/archive, ensure archive commit, then merge
        ensure_archive_commit(change_id, &workspace.path, ...).await?;
        // ... merge to base branch
    }
    WorkspaceState::Applied | WorkspaceState::Applying { .. } | WorkspaceState::Created => {
        // Continue with apply (or resume from iteration)
        execute_apply_and_archive_parallel(&workspaces, ...).await?;
    }
}
```

### Benefits

1. **Idempotency**: Running the orchestrator multiple times produces the same result
2. **Manual Intervention Support**: Detects manually archived or merged changes
3. **Resume Correctness**: Interrupted operations resume from the correct step
4. **No Duplicate Work**: Skips completed operations automatically

## Dynamic Queue Immediate Execution (TUI Mode)

### Overview

In TUI mode, when users add changes to the queue via Space key during parallel batch execution, the newly queued items now start executing immediately when execution slots are available, rather than waiting for the current batch to complete.

### Implementation

**Location**: `src/parallel/mod.rs` - `execute_with_reanalysis` method

The parallel executor polls the dynamic queue at the beginning of each iteration of the main execution loop:

1. **Queue Polling**: Checks `DynamicQueue::pop()` for newly added change IDs
2. **Change Loading**: Loads change details via `openspec::list_changes_native()`
3. **Injection**: Adds new changes to the pending changes vector
4. **Re-analysis**: New changes go through dependency analysis with remaining changes
5. **Debounce**: Updates the queue change timestamp to trigger debounce logic

### Configuration

The dynamic queue is optional and only enabled in TUI mode:

- **TUI Mode**: `DynamicQueue` is passed to `ParallelExecutor` via `set_dynamic_queue()`
- **CLI Mode**: No `DynamicQueue` provided, executor operates normally

### Debounce Logic

Queue changes trigger a 10-second debounce period to prevent excessive re-analysis:

- New items are added to the pending set immediately
- Re-analysis only occurs after the debounce period expires
- Allows multiple changes to be queued before expensive dependency analysis

### Event Flow

When a change is dynamically added:

1. `ParallelEvent::Log` - "Dynamically added to parallel execution: {change_id}"
2. Queue change timestamp updated (triggers debounce)
3. `ParallelEvent::AnalysisStarted` - When re-analysis begins (after debounce)
4. Standard execution events (`WorkspaceCreated`, `ApplyStarted`, etc.)

### Benefits

1. **Immediate Feedback**: Users see changes progress from "Queued" to "Analyzing"/"Processing" when slots are available
2. **Better Resource Utilization**: Available parallelism capacity is used instead of sitting idle
3. **Seamless UX**: No need to wait for batch completion to start new work

### Troubleshooting

**Issue**: Newly queued items not starting immediately
- **Cause**: Debounce period active or all slots occupied
- **Solution**: Wait for debounce period (10 seconds) or for a slot to become available

**Issue**: Multiple re-analysis cycles for single queue addition
- **Cause**: Queue changes triggering debounce repeatedly
- **Solution**: Expected behavior; debounce prevents excessive re-analysis overhead

## JJ Merge Guidance

- Keep merge commits empty: use `jj new --no-edit` and then `jj edit <merge_rev>` followed by `jj new <merge_rev>`.
- Avoid `jj workspace update-stale` in merge flows so working-copy changes stay out of the merge commit.
