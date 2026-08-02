# Conflux

[![日本語](https://img.shields.io/badge/%E8%A8%80%E8%AA%9E-日本語-0f766e?style=flat-square)](./README.ja.md)
[![English](https://img.shields.io/badge/Language-English-2563eb?style=flat-square)](./README.md)
[![简体中文](https://img.shields.io/badge/%E8%AF%AD%E8%A8%80-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-dc2626?style=flat-square)](./README.zh-CN.md)
[![Español](https://img.shields.io/badge/Idioma-Espa%C3%B1ol-f59e0b?style=flat-square)](./README.es.md)
[![Português (BR)](https://img.shields.io/badge/Idioma-Portugu%C3%AAs%20(BR)-16a34a?style=flat-square)](./README.pt-BR.md)
[![한국어](https://img.shields.io/badge/%EC%96%B8%EC%96%B4-%ED%95%9C%EA%B5%AD%EC%96%B4-7c3aed?style=flat-square)](./README.ko.md)
[![Français](https://img.shields.io/badge/Langue-Fran%C3%A7ais-0891b2?style=flat-square)](./README.fr.md)
[![Deutsch](https://img.shields.io/badge/Sprache-Deutsch-4b5563?style=flat-square)](./README.de.md)
[![Русский](https://img.shields.io/badge/%D0%AF%D0%B7%D1%8B%D0%BA-%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-b91c1c?style=flat-square)](./README.ru.md)
[![Tiếng Việt](https://img.shields.io/badge/Ng%C3%B4n%20ng%E1%BB%AF-Ti%E1%BA%BFng%20Vi%E1%BB%87t-ea580c?style=flat-square)](./README.vi.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![Conflux TUI](docs/images/conflux-tui.jpg)

Conflux is a tool that orchestrates autonomous development by AI coding agents based on specification-driven development. Without requiring continuous human supervision, it keeps changes moving through a full workflow: application, acceptance judgment, archiving, and final merge.

The goal is not one-off code generation. It is to define the specification first, then continuously grow a production-minded, substantial finished product by stacking changes that follow that specification.

Conflux is also not tied to any specific AI vendor. It is designed so you can swap tools such as [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), and [OpenCode](https://opencode.ai/).

## Core Concepts of Conflux

- **Autonomous development that keeps moving while you sleep**: Even without constant human attention, AI agents process changes one by one and keep development moving forward.
- **Specification-driven development**: Using [OpenSpec](https://github.com/openspec/openspec), you define the specification first, then proceed with implementation, acceptance, and improvement based on it.
- **Continuously growing a substantial finished product**: Instead of stopping at one-off generation, Conflux accumulates changes over time and steadily moves closer to a finished product.

## Mechanisms That Make It Work

- **Multi-layer Ralph loops**: Conflux improves through repeated iteration while keeping the context handed off in each iteration as small as possible, making LLM usage more efficient.
- **Parallel development with git worktree**: By assigning an independent worktree to each change, Conflux enables multiple changes to proceed safely in parallel.
- **Vendor-independent agent choice**: Conflux is not locked to any specific vendor such as [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://openai.com/index/openai-codex/), or [OpenCode](https://opencode.ai/). You can swap implementation and evaluation agents depending on the task.
- **Separation of implementation and acceptance roles**: By separating the role that drives implementation from the role that evaluates the result, you can combine a fast coder with a smarter reviewer. This improves overall development speed while using LLMs more efficiently.

In short, Conflux is an **orchestrator for running autonomous, specification-driven development as a practical development workflow with parallel execution and clear role separation, continuously pushing a substantial finished product forward**.

## Main Usage

| Usage | Command |
|------|---------|
| TUI | `cflx` |
| Headless execution | `cflx run` |

Useful TUI keys:

| Key | Action |
|-----|--------|
| `Space` | Mark or unmark changes |
| `F5` | Start, resume, retry, or continue processing |
| `x` | Queue eligible `not queued` changes while processing is running |

For server mode, remote TUI, REST API, and `cflx service`, see the [Server Mode Guide (English)](docs/guides/SERVER.md).

## Quick Start

For initial setup, see [QUICKSTART.md](QUICKSTART.md).

## Basic Commands

```bash
# TUI
cflx

# Headless execution
cflx run

# Run only a specific change
cflx run --change add-feature-x

# Initialize the configuration file
cflx init

# Install bundled skills
cflx install-skills

# Install bundled skills for Claude Code
cflx install-skills --claude

# Install bundled skills globally for Claude Code
cflx install-skills --claude --global
```

## Configuration

The configuration file format is JSONC.

- `.cflx.jsonc`
- `~/.config/cflx/config.jsonc`
- `--config <PATH>`

TUI user preferences are intentionally separate from orchestration config. The default start/resume/retry/continue key is `F5`; override only the local TUI start binding in `~/.config/cflx/tui.jsonc`:

```jsonc
{
  "keybindings": {
    "start": ["F5", "!"]
  }
}
```

Generate config templates:

```bash
cflx init
cflx init --template opencode
cflx init --template codex
cflx init --force
```

For detailed configuration examples, hooks, workspace execution, and command queue explanations, see [docs/guides/USAGE.md](docs/guides/USAGE.md).

## Installation

```bash
cargo install cflx
```

## Documentation

| Document | Description |
|----------|-------------|
| [QUICKSTART.md](QUICKSTART.md) | Initial setup |
| [Server Mode Guide (English)](docs/guides/SERVER.md) | Server mode, remote TUI, the `/api/v2` web operator console, background service |
| [README.ja.md](README.ja.md) | Full documentation (Japanese) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Usage examples |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Development guide |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Release guide |
| [docs/openapi.yaml](docs/openapi.yaml) | API specification |

## License

MIT
