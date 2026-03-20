# Contributing

Thanks for contributing to Conflux.

## Development Setup

Prerequisites:

- Rust 1.70 or later
- Cargo
- `prek` for Git hooks (recommended)

Build and test locally:

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Format, lint, and test
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Useful extras:

```bash
# Coverage
cargo llvm-cov --all-features

# Run with debug logging
RUST_LOG=debug cargo run -- run
```

## Git Hooks

This project uses [prek](https://prek.j178.dev/) for Git hooks.

If you previously used `pre-commit`, uninstall it first:

```bash
pre-commit uninstall
```

Install and enable hooks:

```bash
brew install prek
prek install
```

Common commands:

```bash
# Run all hooks on all files
prek run --all-files

# Run selected hooks
prek run rustfmt clippy

# List available hooks
prek list
```

Hook configuration lives in `.pre-commit-config.yaml`. Running `prek run --all-files` also runs `make openapi` and stages `docs/openapi.yaml`.

## Project Structure

High-level layout:

```text
src/
  main.rs            # CLI entry point
  cli.rs             # Command-line parsing
  orchestrator.rs    # Main orchestration loop
  agent/             # AI agent execution
  config/            # Configuration loading and defaults
  execution/         # Apply/archive execution logic
  orchestration/     # Shared orchestration steps
  parallel/          # Parallel execution and workspaces
  remote/            # Remote server client
  server/            # Multi-project server daemon
  tui/               # Terminal UI
  vcs/               # VCS abstraction and git backend
  web/               # Web monitoring
tests/               # Integration and end-to-end tests
```

For a broader walkthrough, see `docs/guides/DEVELOPMENT.md`.

## Contribution Notes

- Keep user-facing usage and product overview in `README.md`.
- Put contributor workflow, build/test steps, and repository internals in `CONTRIBUTING.md`.
- When adding CLI features, update both `README.md` and `README.ja.md` if the user-facing behavior changes.
- When changing release or API behavior, update the relevant docs under `docs/`.
