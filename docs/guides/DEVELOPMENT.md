# Development Guide

This document provides information for developers contributing to Conflux.

## Prerequisites

- Rust 1.70 or later
- Cargo (included with Rust)

## Building

### Debug build

```bash
cargo build
```

### Release build

```bash
cargo build --release
```

The binary will be available at `target/release/cflx`.

## Testing

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
```

### Run specific tests

```bash
# Run tests matching a pattern
cargo test test_name

# Run tests in a specific module
cargo test module_name::
```

## Debugging

### Run with logging

```bash
RUST_LOG=debug cargo run -- run
```

Available log levels: `error`, `warn`, `info`, `debug`, `trace`

### Run with specific log targets

```bash
# Log only orchestrator module
RUST_LOG=conflux::orchestrator=debug cargo run -- run

# Log multiple modules
RUST_LOG=conflux::agent=debug,conflux::hooks=debug cargo run -- run
```

## Project Structure

```
src/
├── main.rs           # Entry point (default: TUI mode)
├── cli.rs            # CLI argument parsing (run, tui, init)
├── config.rs         # Configuration file parsing (JSONC)
├── agent.rs          # AI agent runner (configurable commands)
├── history.rs        # Apply attempt history tracking
├── hooks.rs          # Lifecycle hooks execution
├── jj_workspace.rs   # Parallel execution with jj workspaces
├── templates.rs      # Configuration templates (claude, opencode, codex)
├── task_parser.rs    # Task file parsing and progress calculation
├── error.rs          # Error types (OrchestratorError)
├── openspec.rs       # OpenSpec wrapper (list, archive)
├── opencode.rs       # OpenCode runner (legacy, kept for compatibility)
├── progress.rs       # Progress display (indicatif)
├── tui.rs            # Interactive TUI dashboard (ratatui)
└── orchestrator.rs   # Main orchestration loop
```

## Architecture Overview

### Core Components

| Component | File | Responsibility |
|-----------|------|----------------|
| CLI | `cli.rs` | Parse command-line arguments and dispatch to subcommands |
| Config | `config.rs` | Load and parse JSONC configuration files |
| Agent | `agent.rs` | Execute AI agent commands with placeholder substitution |
| History | `history.rs` | Track apply attempts per change for retry context |
| Hooks | `hooks.rs` | Execute lifecycle hooks at various workflow stages |
| JjWorkspace | `jj_workspace.rs` | Manage jj workspaces for parallel execution |
| Orchestrator | `orchestrator.rs` | Main loop: list changes, select next, apply/archive |
| TUI | `tui.rs` | Interactive terminal dashboard using ratatui |
| OpenSpec | `openspec.rs` | Wrapper for OpenSpec CLI commands |

### Data Flow

```
User starts orchestrator
        ↓
    Load config (.cflx.jsonc)
        ↓
    Run on_start hook
        ↓
┌─→ List changes (openspec list)
│       ↓
│   Select next change
│   • Priority 1: 100% complete → archive
│   • Priority 2: LLM analysis
│   • Priority 3: Highest progress
│       ↓
│   Execute apply_command or archive_command
│       ↓
│   Run post hooks
│       ↓
└── Repeat until no changes remain
        ↓
    Run on_finish hook
```

## Code Style

### Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

### Pre-commit checks

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Install the Git hook using prek:

**Migration from pre-commit:**

If you were previously using pre-commit, uninstall it first:

```bash
pre-commit uninstall
```

**Setup prek:**

```bash
# Install prek
brew install prek

# Install hooks
prek install
```

The prek hook configuration is defined in `.pre-commit-config.yaml` (prek is fully compatible with pre-commit configuration format). When you run `prek run --all-files`, it auto-runs `make openapi` and stages `docs/openapi.yaml`.

## Adding New Features

### Adding a new hook

1. Add the hook variant to `HookType` enum in `hooks.rs`
2. Add the field to `HooksConfig` struct in `config.rs`
3. Call `execute_hook()` at the appropriate place in `orchestrator.rs`
4. Update templates in `templates.rs` with commented example
5. Document in README.md

### Adding a new configuration option

1. Add the field to `OrchestratorConfig` in `config.rs`
2. Handle the option in the relevant component
3. Update templates in `templates.rs`
4. Document in README.md

### Adding a new CLI subcommand

1. Add the subcommand to `Commands` enum in `cli.rs`
2. Create argument struct if needed (e.g., `RunArgs`)
3. Handle the subcommand in `main.rs`
4. Document in README.md and README.ja.md

### Adding support for a new VCS for parallel execution

1. Create a new module (e.g., `git_worktree.rs`) similar to `jj_workspace.rs`
2. Implement workspace creation, merge, and cleanup operations
3. Add detection logic in orchestrator to choose the appropriate backend
4. Update documentation

## Command Template Escaping

- Placeholders like `{prompt}`, `{proposal}`, and `{conflict_files}` are shell-escaped with `shlex` on POSIX systems.
- Prefer templates without manual quotes, such as `opencode run --format json {prompt}`.
- If a placeholder must live inside a larger single-quoted string (for example `/openspec:apply {change_id} {prompt}`), keep the surrounding quotes; the expander strips extra outer quotes to avoid double-quoting.
- Windows builds keep the existing behavior and sanitize null/newline characters.

## Release Process

1. Update version in `Cargo.toml`
2. Run all tests: `cargo test`
3. Run clippy: `cargo clippy -- -D warnings`
4. Build release: `cargo build --release`
5. Tag the release: `git tag vX.Y.Z`

## Troubleshooting Development Issues

### Build fails with dependency errors

```bash
cargo update
cargo build
```

### Tests fail intermittently

Some tests may depend on timing. Run with `--test-threads=1`:

```bash
cargo test -- --test-threads=1
```

### TUI doesn't render correctly

Ensure your terminal supports:
- 256 colors
- Unicode characters
- Alternate screen buffer

Test with: `echo $TERM` (should be `xterm-256color` or similar)
