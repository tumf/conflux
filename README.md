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
cflx client mark add-my-change --json     # select it; unrelated marks are preserved
cflx client start --json                  # F5 equivalent over the authoritative marks
cflx client force-stop-change add-my-change --json   # kill exactly this one, now
cflx client wait add-my-change --json     # waits for as long as the work takes
cflx client mcp                           # serve the same controls over stdio MCP
```

The verbs are the operator's own: `status`, `mark`, `unmark`, `start`, `stop`,
`force-stop`, `force-stop-change`, `wait`, and the `subscribe` group, plus `mcp`
for hosts that speak
the protocol instead of a shell. `--project-dir ABSOLUTE_PATH` names any
directory inside the project's Git working tree and is the normal explicit route;
`--unix-socket PATH` is the low-level override of the default
`${GIT_COMMON_DIR}/cflx-api.sock`. The two conflict at parse time, and
`--auth-token-env NAME` names an environment variable holding the bearer token —
a token value is never accepted in argv and never printed.

**A mark is selection, not admission.** `mark` and `unmark` are target-scoped
desired-state writes: they set exactly the named proposals' execution marks,
leave every unrelated mark alone, submit no queue intent, start nothing, and
return once the commands settle. Whether marked work then runs is the owner's own
settlement and analysis, exactly as it is for a mark typed at the TUI. `start`,
`stop`, and `force-stop` submit the same shared lifecycle intents F5/`!` and the
stop controls submit — `start` consumes the owner's authoritative mark set and
takes no target list, because "start only these" is not something the shared
transaction can express.

**`force-stop-change` kills exactly one proposal.** It is the target-scoped
counterpart of process-wide `force-stop`, and the only control that bypasses the
graceful SIGTERM escalation window `stop` gives a change: the owner sends SIGKILL
straight to the managed process group that proposal owns, waits for confirmed
reaping, then clears its queue admission and execution mark together so later
mark settlement cannot redispatch it. Unrelated changes keep their processes,
marks, queue intent, execution IDs, and subscriptions, and the process-wide run
mode, scheduler state, and stop state do not change. Completed worktree
effects — an Apply commit that already landed — are preserved, and the settled
result says so with `effects_rolled_back: false`. Its success token is `stopped`,
not `accepted`, because one settled command really did end one execution
episode. The target's own row settles as terminal `stopped` rather than idle `not
queued`, which is what releases a concurrent `wait` with `change_requires_action`
instead of leaving it observing a proposal nobody will advance. A target that is admitted but owns no live process is dequeued outright;
a terminal, unadmitted, merge-wait, or resolve-wait target is refused with
`target_ineligible`, and the owner publishes the same fact ahead of time as
`actions.force_stop_change`. `stop_and_dequeue` keeps its own
cancel-confirm-dequeue contract unchanged.

**`wait` has no deadline unless you ask for one.** `--timeout` defaults to `0`,
and zero in any unit means "wait for as long as the work takes": the wait ends at
verified completion, a typed failure, owner replacement, or your own
cancellation. A positive `--timeout D` opts back into one operation deadline
whose expiry is the typed `timeout` outcome. Neither form makes a subprocess
unbounded — each owner request keeps the transport's per-request valve, and every
Git child an unbounded wait spawns is terminated and reaped at a finite
per-invocation deadline of its own rather than reported as `timeout`.

**Read `outcome`, not prose.** `--json` prints exactly one versioned envelope on
stdout; diagnostics go to stderr, and each outcome has its own stable exit
status. The successes are narrow: `observed`, `marked`, `unmarked`, `unchanged`,
`stopped`, `accepted`, `subscribed`, `cleared`, and `completed`. Everything else —
`owner_not_running`, `owner_not_command_capable`, `owner_restarted`,
`change_not_found`, `target_ineligible`, `revision_conflict`,
`transport_not_permitted`, `unsupported_owner`, `partial_intent`,
`observation_conflict`, `evidence_error`, `change_rejected`,
`change_requires_action`, `process_failed`, `timeout`, `usage_error` — is a
non-zero refusal.

**`wait` waits for the owner, not for you.** It holds while the owner can still
advance the change on its own — `not queued`, `queued`, `blocked`, `applying`,
`accepting`, `rejecting`, `archiving`, `resolving` — and releases the moment the
row is one only a new operator action can move: `error`, `merge wait`, `stopped`,
and `stalled` return `change_requires_action` (exit `27`) carrying
`detail.observed_status`, any `detail.error_detail` the owner published, and
`detail.commands_submitted: 0`. `rejected` keeps its own `change_rejected`.
`blocked` is the one row the status alone cannot classify: a dependency wait is
work the owner clears by itself and keeps the wait observing, while a validated
external prerequisite is a hold the owner already handed back, so it releases
with the same `change_requires_action` and publishes the blocker's
`unblock_condition` and `prerequisite_owner` in `detail.blocker`. The
classification runs on the first observation too, so waiting on an already-parked
change reports it immediately instead of hanging. A settled `merged`, `pushed`,
or `archived` row still has to be certified from the repository; it gets one
bounded re-observation for evidence that landed a moment late, and then either
`completed` or the same `change_requires_action`.

**Accepted is not completion.** A settled control command proves the owner took
the intent, nothing more, and a settled mark proves less still. `wait` is the
observation-only counterpart: it submits no command and returns `completed` only
when current Git/OpenSpec evidence proves the owner's declared terminal mode.
Releasing on a parked row is not a repair: nothing is started, retried,
resolved, merged, or cleaned up on the way out.

### `cflx client mcp`

`cflx client mcp` is a stdio Model Context Protocol server over exactly that
boundary, with three closed tools: `cflx_status`, `cflx_control`, and
`cflx_subscribe`. `cflx_control` takes one action — `mark`, `unmark`, `start`,
`stop`, `force_stop`, or `force_stop_change` — and each tool calls the same
module its command does. The two mark actions take 1 through 64 distinct
`change_ids`, `force_stop_change` takes exactly one, and the three process-wide
lifecycle actions take none.
It exposes no raw `/api/v2` command construction, so a model cannot name a
command type, an expected revision, an idempotency key, queue intent, or shell
source. stdout carries JSON-RPC frames and nothing else.

Every tool accepts an optional absolute `project_dir` and an optional low-level
`unix_socket`, so one server process registered globally can drive any number of
projects by naming one per call. Nothing is remembered between calls.

`cflx_wait` is deliberately absent from MCP: a completion wait is open for as
long as the work takes, which is not a tool call. `cflx client wait` remains the
CLI oracle, and an MCP host that wants asynchronous completion registers an
explicit callback with `cflx_subscribe`. A host that cannot execute a callback
has no MCP completion oracle, by design.

### Completion notifications

Nothing is registered for you. An agent that wants to be told when a proposal
finishes asks, explicitly — a shell through `cflx client subscribe`, an MCP host
through `cflx_subscribe`. Neither requires the other.

```bash
I=$(cflx client status --json | jq -r .instance_id)
cflx client subscribe set add-my-change --instance-id "$I" --json -- /absolute/callback --flag v
cflx client subscribe get add-my-change --instance-id "$I" --json
cflx client subscribe clear add-my-change --instance-id "$I" --json
```

A subscription is keyed by the *proposal*, not by an execution episode, so it can
be registered before anything is admitted — which is the gap that used to be
closed by inferring a registration from an admission result. One request names 1
through 64 distinct proposals. Everything after `--` is the callback argv, one
element per argument exactly as typed; the CLI never parses shell source.
`--blocked` opts into the non-terminal attention edge.

Whenever a subscribed proposal enters a new execution episode, the owner binds
that episode and runs the argv **once** when it reaches a typed terminal
classification (`completed`, `failed`, `stopped`), with `owner_stopping` on
graceful shutdown. Re-admission after a retry is a distinct episode and a
distinct notification. This exists because the TUI stays alive after the work
finishes, so process exit was never a completion signal.

- **Delivery notifies; it does not resume.** Conflux runs the registered argv and
  draws no conclusion from it. It starts no agent, resumes no session, and a
  callback's exit status changes no workflow outcome. Whatever happens next is
  external and explicit.
- `completed` uses the same repository oracle `cflx client wait` certifies with.
  A change disappearing from the owner's snapshot is never completion.
- Delivery dedupe is keyed by the execution episode, so replacing a subscription,
  clearing it, or clearing and setting it again never replays a terminal event
  this owner already delivered. Registering *after* the latest episode settled
  delivers that event immediately, once.
- The callback is argv, not shell source — no `sh -c`, no quoting, no expansion —
  and its environment is *replaced* with exactly `CFLX_EVENT_PATH`,
  `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and
  `CFLX_INSTANCE_ID`. No owner token or configuration reaches it.
- Registration is accepted **only over the owner's Unix socket**. An
  authenticated TCP client is refused with `transport_not_permitted`, and it is
  not told the registered argv on a read either — it sees presence, execution
  state, and delivery history instead. Every request, inspection included,
  carries the complete `(instance_id, change_id)` binding.
- The event payload is created `0400` inside a `0700` owner-private directory, so
  a callback cannot open `CFLX_EVENT_PATH` for writing by default. That is
  default mutation refusal, not an integrity guarantee against a same-UID
  callback — what makes it safe is that the owner writes the file once and
  never reads it back. It is removed once the callback is reaped.
- Callback stdout and stderr are drained for the whole life of the callback and
  retained only up to a fixed ceiling, so a chatty callback neither blocks on a
  full pipe nor grows the owner. Overflow is a truncation diagnostic and never
  terminates the callback; only the runtime ceiling and shutdown cancellation do,
  and both kill and reap.
- Delivery stays serialized, including through graceful shutdown: one finite
  deadline covers every queued or running callback, nothing new starts after it,
  and no event artifact is removed until its callback has been reaped.
- Subscriptions are process-local and die with the owner: nothing here is durable
  workflow state, an owner restart invalidates every registration, and delivery
  failure cannot change any outcome.

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
| [docs/guides/VERIFICATION_EVIDENCE.md](docs/guides/VERIFICATION_EVIDENCE.md) | `cflx openspec verify`, bound evidence reuse, and why a rerun is never a verdict |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide |
| [docs/guides/DEVELOPMENT.md](docs/guides/DEVELOPMENT.md) | Development guide |
| [docs/guides/RELEASE.md](docs/guides/RELEASE.md) | Release guide |
| `cflx openapi` / `GET /api/v2/openapi.yaml` | Canonical `/api/v2` contract (generated at runtime; not tracked in the repository) |

## License

MIT
