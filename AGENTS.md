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

# AGENTS.md - OpenSpec Orchestrator

This document provides essential information for AI coding agents working on this Rust codebase.

## Project Overview

OpenSpec Orchestrator automates the OpenSpec change workflow (list -> dependency analysis -> apply -> archive). It orchestrates `openspec` and `opencode` CLI tools to process changes autonomously.

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

## Code Style Guidelines

### Imports

- Use `crate::` prefix for internal module imports
- Group imports: std -> external crates -> internal modules
- Use specific imports, avoid glob imports

```rust
use crate::error::{OrchestratorError, Result};
use regex::Regex;
use std::process::Command;
use tracing::{debug, info};
```

### Error Handling

- Use `thiserror` for error type definitions
- Define custom `Result<T>` type alias
- Use `?` operator for error propagation
- Use `map_err` for error context conversion

```rust
#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("OpenSpec command failed: {0}")]
    OpenSpecCommand(String),
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;
```

### Naming Conventions

- Types: `PascalCase` (e.g., `OrchestratorState`, `OpenCodeRunner`)
- Functions/methods: `snake_case` (e.g., `list_changes`, `run_command`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `STATE_FILE`)
- Modules: `snake_case` (e.g., `openspec.rs`, `orchestrator.rs`)

### Struct Definitions

- Use `#[derive(...)]` for common traits
- Place `#[allow(dead_code)]` on unused fields if intentional
- Use doc comments for public APIs

```rust
#[derive(Debug, Clone)]
pub struct Change {
    pub id: String,
    pub completed_tasks: u32,
    pub total_tasks: u32,
    #[allow(dead_code)]
    pub last_modified: String,
}
```

### Async Code

- Use `tokio` runtime with `#[tokio::main]`
- Use `async/await` for async functions
- Prefer `tokio::process::Command` for async process execution

### Logging

- Use `tracing` crate for structured logging
- Log levels: `error!`, `warn!`, `info!`, `debug!`
- Initialize with `tracing_subscriber`

```rust
use tracing::{debug, error, info, warn};

info!("Starting orchestrator");
debug!("OpenCode command exited with status: {:?}", status);
error!("Archive failed for {}: {}", id, e);
```

### Testing

- Unit tests in `#[cfg(test)]` modules within source files
- Integration tests in `tests/` directory
- Use `tempfile` crate for test fixtures
- Mock external commands with shell scripts for E2E tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        let result = some_function();
        assert_eq!(result, expected);
    }
}
```

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

- Project config: `.openspec-orchestrator.jsonc` (JSONC format with comments)
- Global config: `~/.openspec-orchestrator.jsonc`

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
| **Archived** | Archive commit exists: `Archive: <change_id>` (not in main) | Skip apply/archive, run merge only |
| **Merged** | Archive commit found in main branch | Skip all operations, cleanup workspace |

### State Detection Module

**Location**: `src/execution/state.rs`

**Key Functions**:

- `detect_workspace_state(change_id, repo_root)` - Main entry point, returns `WorkspaceState`
- `is_merged_to_main(change_id, repo_root)` - Check if archive commit is in main branch
- `has_apply_commit(change_id, repo_root)` - Check for apply completion
- `get_latest_wip_snapshot(change_id, repo_root)` - Get highest WIP iteration number
- `is_archive_commit_complete(change_id, repo_root)` - Verify archive commit and clean working tree

### Integration

**Location**: `src/parallel/mod.rs` - `execute_group()` function

When resuming a workspace:

```rust
let workspace_state = detect_workspace_state(change_id, &workspace.path).await?;

match workspace_state {
    WorkspaceState::Merged => {
        // Skip all operations, cleanup workspace
        cleanup_workspace(&workspace.name).await?;
    }
    WorkspaceState::Archived => {
        // Skip apply/archive, ensure archive commit, then merge
        ensure_archive_commit(change_id, &workspace.path, ...).await?;
        // ... merge to main
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

## JJ Merge Guidance

- Keep merge commits empty: use `jj new --no-edit` and then `jj edit <merge_rev>` followed by `jj new <merge_rev>`.
- Avoid `jj workspace update-stale` in merge flows so working-copy changes stay out of the merge commit.
