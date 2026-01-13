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
  history.rs            # Apply context history management
  hooks.rs              # Lifecycle hook execution
  parallel_run_service.rs # Parallel execution service
  task_parser.rs        # Native task.md parser
  templates.rs          # Configuration templates

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

## Configuration Files

- Project config: `.openspec-orchestrator.jsonc` (JSONC format with comments)
- Global config: `~/.openspec-orchestrator.jsonc`

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

## JJ Merge Guidance

- Keep merge commits empty: use `jj new --no-edit` and then `jj edit <merge_rev>` followed by `jj new <merge_rev>`.
- Avoid `jj workspace update-stale` in merge flows so working-copy changes stay out of the merge commit.
