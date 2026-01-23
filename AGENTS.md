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

Essential information for AI coding agents working on this Rust codebase.

## Project Overview

Conflux automates the OpenSpec change workflow (list → dependency analysis → apply → acceptance → archive → resolve → merged). It orchestrates `openspec` and AI coding agent tools to process changes autonomously.

## Commands

```bash
# Build
cargo build                    # debug
cargo build --release          # release

# Lint
cargo fmt --check              # check formatting
cargo fmt                      # apply formatting
cargo clippy -- -D warnings    # lints with warnings as errors

# Test
cargo test                     # all tests
cargo test <name>              # single test by name
cargo test -- --nocapture      # with output
cargo test --test e2e_tests    # specific test file

# Run
RUST_LOG=debug cargo run -- run --dry-run
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
  analyzer.rs           # Change dependency analyzer
  approval.rs           # Change approval management
  command_queue.rs      # Command queue with stagger and retry
  history.rs            # Apply/archive/resolve history
  hooks.rs              # Lifecycle hook execution
  parallel_run_service.rs # Parallel execution service
  task_parser.rs        # Native tasks.md parser
  templates.rs          # Configuration templates

  execution/            # Shared execution logic
    apply.rs            # Apply operation logic
    archive.rs          # Archive operation logic
    state.rs            # Workspace state detection
    types.rs            # Common type definitions

  config/               # Configuration
    defaults.rs         # Default values
    expand.rs           # Environment variable expansion
    jsonc.rs            # JSONC parser

  vcs/                  # Version Control abstraction
    commands.rs         # Common VCS interface
    git/                # Git backend

  parallel/             # Parallel execution
    executor.rs         # Parallel change executor
    events.rs           # Progress reporting events
    conflict.rs         # Conflict detection/resolution
    cleanup.rs          # Workspace cleanup

  tui/                  # Terminal User Interface
    render.rs           # Terminal rendering
    runner.rs           # TUI main loop
    state/              # State management
      change.rs         # Change state
      events.rs         # Event handling
      logs.rs           # Log state
      modes.rs          # Mode state

tests/
  e2e_tests.rs          # End-to-end tests
  ralph_compatibility.rs # Ralph plugin tests
```

## Code Style

### Imports

Group in order: `std` → external crates → `crate::`

```rust
use std::path::PathBuf;

use regex::Regex;
use tracing::{debug, info};

use crate::error::{OrchestratorError, Result};
```

### Error Handling

- Use `thiserror` for error definitions
- Use `?` operator for propagation
- Use `#[from]` for automatic conversions

```rust
#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;
```

### Naming

| Type | Convention | Example |
|------|------------|---------|
| Types/Structs/Enums | PascalCase | `OrchestratorState` |
| Functions/Methods | snake_case | `list_changes` |
| Constants | SCREAMING_SNAKE_CASE | `STATE_FILE` |
| Modules | snake_case | `command_queue.rs` |

### Async

- Use `tokio` runtime with `#[tokio::main]` or `#[tokio::test]`
- Prefer `tokio::process::Command` over `std::process::Command`
- Use `tokio::time::sleep` over `std::thread::sleep`

### Logging

Use `tracing` crate (not `println!`):

```rust
info!("Starting orchestrator");
debug!(status = ?exit_status, "Agent command exited");
error!(error = %e, "Failed to execute command");
```

### Testing

- Unit tests: `#[cfg(test)]` modules in source files
- Integration tests: `tests/` directory
- Use `tempfile` for temporary files
- Use `#[tokio::test]` for async tests

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| tokio | Async runtime |
| clap | CLI parsing |
| serde/serde_json | Serialization |
| thiserror | Error definitions |
| tracing | Logging |
| indicatif | Progress bars |
| async-trait | Async traits |
| nix (Unix) | Process groups |
| windows (Windows) | Job objects |

## Configuration

- Project config: `.cflx.jsonc`
- Global config: `~/.cflx.jsonc`

### Workspace Base Directory

**Key**: `workspace_base_dir`

Default locations (when not configured):
- **macOS**: `~/Library/Application Support/conflux/worktrees/<project_slug>`
- **Linux**: `~/.local/share/conflux/worktrees/<project_slug>`
- **Windows**: `%APPDATA%\Conflux\worktrees\<project_slug>`

Project slug format: `<repo_basename>-<hash8>` (e.g., `conflux-a1b2c3d4`)

## Key Concepts

### Command Execution Queue

**Module**: `src/command_queue.rs`

Prevents resource conflicts with:
1. **Staggered Start**: Configurable delay between commands (`command_queue_stagger_delay_ms`, default: 2000ms)
2. **Automatic Retry**: Retries on transient errors (module resolution, network issues)

### Workspace State Detection

**Module**: `src/execution/state.rs`

Enables idempotent resume in parallel mode:

| State | Detection | Action |
|-------|-----------|--------|
| Created | No commits | Start apply |
| Applying | WIP commits exist | Resume from iteration |
| Applied | Apply commit exists | Run archive only |
| Archived | Archive commit exists | Run merge only |
| Merged | In base branch | Cleanup workspace |

### Retry Context History

**Module**: `src/history.rs`

Tracks apply/archive/resolve attempts in memory, injected into AI agent prompts for learning from previous failures.

### Process Management

Platform-specific child process cleanup:
- **Unix**: Process groups with `setpgid()` + `killpg()`
- **Windows**: Job objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`

### TUI Worktree View

Press `Tab` to switch between Changes and Worktrees views. Features:
- Parallel conflict detection in background
- Branch merge with `M` key (conflict-free only)
- Worktree management: create (`+`), delete (`D`), open editor (`e`)

### Dynamic Queue (TUI Mode)

**Module**: `src/parallel/mod.rs`

Changes added via Space key during execution start immediately when slots available. Uses 10-second debounce for re-analysis.
