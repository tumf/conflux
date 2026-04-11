# Conflux

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Conflux is a CLI that automates the OpenSpec change workflow. It coordinates `openspec` and AI coding agents to apply, accept, and archive changes.

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
