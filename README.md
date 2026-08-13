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

`cflx`, `cflx tui`, and `cflx run` serve `/api/v2` on a repository-scoped Unix
socket at `${GIT_COMMON_DIR}/cflx-api.sock` by default — no TCP port, no flag:

```bash
curl --unix-socket "$(git rev-parse --git-common-dir)/cflx-api.sock" http://localhost/api/v2/state
```

Use `--web-unix-socket PATH` to move it, `--no-web-unix-socket` to turn it off,
and `--web` to additionally start the browser-facing TCP Web UI. For the web
monitoring UI, REST API, and `/api/v2`, see the
[Web UI Guide (English)](docs/guides/WEBUI.md).

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

## Delegating to an existing owner

When a Conflux process already holds this repository, another agent hands work to
it with `cflx client` — a **client**, never a second owner. It takes no
repository lock, binds no listener, starts no run, and writes nothing to the
workspace. That is the whole difference from `cflx run`, which *is* an owner and
would contend for the lock with the process you meant to talk to.

```bash
cflx client status --json                 # read the owner; mutates nothing
cflx client enqueue add-my-change --json  # ask the owner to admit one change
cflx client wait add-my-change --timeout 45m --json
cflx client mcp                           # serve the same intents over stdio MCP
```

Four commands, and only four. `--unix-socket PATH` overrides the default
`${GIT_COMMON_DIR}/cflx-api.sock`, and `--auth-token-env NAME` names an
environment variable holding the bearer token — a token value is never accepted
in argv and never printed.

**Admission is not completion.** A successful `enqueue` proves only that the
owner accepted the intent. `wait` is the observation-only counterpart, and it
returns `completed` only when current Git/OpenSpec evidence proves the owner's
declared terminal mode. Read the envelope's `outcome`, never prose.

### `cflx client mcp`

`cflx client mcp` is a stdio Model Context Protocol server over exactly that
boundary, with six closed tools: `cflx_status`, `cflx_enqueue`, `cflx_wait`, and
`cflx_notify_set` / `_get` / `_clear`. It exposes no raw `/api/v2` command
construction, so a model cannot name a command type, an expected revision, an
idempotency key, an execution mark, or shell source. stdout carries JSON-RPC
frames and nothing else.

`cflx_enqueue` returns as soon as admission settles and carries an
`execution_id` naming that exact admitted episode — a retry of the same proposal
is a *different* execution. It never holds the tool call open for the life of a
change.

### Completion notifications

`cflx_notify_set` attaches one bounded argv the owner runs **once** when that
execution reaches a typed terminal classification (`completed`, `failed`,
`stopped`), with `blocked` available as an opt-in attention edge and
`owner_stopping` on graceful shutdown. This exists because the TUI stays alive
after the work finishes, so process exit was never a completion signal.

- `completed` uses the same repository oracle `cflx client wait` certifies with.
  A change disappearing from the owner's snapshot is never completion.
- The callback is argv, not shell source — no `sh -c`, no quoting, no expansion —
  and its environment is *replaced* with exactly `CFLX_EVENT_PATH`,
  `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and
  `CFLX_INSTANCE_ID`. No owner token or configuration reaches it.
- Registration is accepted **only over the owner's Unix socket**. An
  authenticated TCP client is refused with `transport_not_permitted`.
- Registrations are process-local and die with the owner: nothing here is
  durable workflow state, and delivery failure cannot change any outcome.

`examples/integrations/opencode-auto-resume/` is an optional reference
integration that wires this into OpenCode. It is repository-distributed and not
part of the published crate.

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
| [Web UI Guide (English)](docs/guides/WEBUI.md) | Web UI, REST API, `/api/v2`, migrating from server mode |
| [README.ja.md](README.ja.md) | Full documentation (Japanese) |
| [docs/guides/USAGE.md](docs/guides/USAGE.md) | Usage examples |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Development guide |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Release guide |
| [examples/integrations/opencode-auto-resume/README.md](examples/integrations/opencode-auto-resume/README.md) | Optional OpenCode auto-resume reference integration |
| `cflx openapi` / `GET /api/v2/openapi.yaml` | Canonical `/api/v2` contract (generated at runtime; not tracked in the repository) |

## License

MIT
