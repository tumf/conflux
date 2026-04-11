# Conflux

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![Conflux TUI](docs/images/conflux-tui.jpg)

Conflux is a tool that orchestrates self-driving AI coding agents around specification-driven development. Without a human constantly supervising it, Conflux keeps changes moving through implementation, acceptance, archival, and the final merge as one continuous flow.

The goal is not one-off code generation. The goal is to define the spec first, then keep building against that spec so a production-minded, non-trivial product can keep moving forward over time.

Conflux is also vendor-agnostic. It is designed so you can swap in agents such as [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), and [OpenCode](https://opencode.ai/).

## Core Concepts

- **Self-driving development that keeps moving while you sleep**: AI agents process changes one by one and keep the project advancing without constant human attention.
- **Specification-driven development**: Using [OpenSpec](https://github.com/openspec/openspec), Conflux defines the spec first, then drives implementation, acceptance, and iteration against that spec.
- **Growing a non-trivial finished product continuously**: Instead of stopping at a single generation step, Conflux accumulates changes and keeps pushing the product toward a finished state.

## How Conflux makes that possible

- **Multi-Ralph loops**: Conflux improves through repeated iterations while keeping the carried context minimal in each pass, which makes LLM usage more efficient.
- **Parallel development with git worktrees**: Conflux assigns an isolated worktree to each change so multiple changes can progress safely in parallel.
- **Vendor-agnostic agent selection**: Conflux is not tied to a single vendor. Agents like [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), and [OpenCode](https://opencode.ai/) can be swapped depending on the role.
- **Separation of implementation and acceptance**: Conflux separates the agent that pushes implementation forward from the agent that inspects and accepts the result. This lets you pair a fast coder with a stronger reviewer, improving both LLM efficiency and overall development speed.

In short, Conflux is an orchestrator for self-driving, specification-driven development: a practical development flow with parallel execution and role separation that keeps a non-trivial product moving forward continuously.

## Main Usage

| Use | Command |
|------|---------|
| TUI | `cflx` |
| Headless run | `cflx run` |

For server mode, remote TUI, REST API, Web UI, and `cflx service`, see the [Server Mode Guide](docs/guides/SERVER.md).

## Quick Start

For first-time setup, see [QUICKSTART.md](QUICKSTART.md).

## Basic Commands

```bash
# TUI
cflx

# Headless run
cflx run

# Run a specific change
cflx run --change add-feature-x

# Initialize configuration
cflx init

# Install bundled skills
cflx install-skills
```

## Configuration

Configuration files use JSONC.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

Generate a template:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

For detailed configuration examples, hooks, workspace execution, and command queue behavior, see the additional guides below.

## Installation

```bash
cargo install cflx
```

## Documentation

| Document | Description |
|----------|-------------|
| [QUICKSTART.md](QUICKSTART.md) | First-time setup |
| [Server Mode Guide](docs/guides/SERVER.md) | Server mode, remote TUI, Web UI, REST API, background service |
| [README.ja.md](README.ja.md) | Japanese translation |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Usage examples |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contributing guide |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Development guide |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Release guide |
| [docs/openapi.yaml](docs/openapi.yaml) | API specification |

## License

MIT
