# cflx auto-resume for OpenCode

An **optional reference integration**. It is not part of the `cflx` crate, is not
installed by `cflx install-skills`, and nothing in Conflux depends on it. Copy it,
read it, change it.

It closes one gap: an agent asks the resident Conflux owner to admit a change,
and then has nothing to wait on. `cflx client wait` would hold a process open for
the whole change; polling burns turns; and the TUI intentionally stays alive
after the work finishes, so process exit is not a signal either.

So the owner pushes. This integration binds one *execution* to the OpenCode
session that asked for it, and lets Conflux call back when that exact execution
reaches a terminal classification.

```text
OpenCode session  ──cflx_enqueue──▶  cflx client mcp  ──▶  resident cflx owner
       ▲                                                        │
       │                                     one bounded callback, once
       └──────────── role=user message ◀──── cflx-resume-session.mjs
```

## What is here

| Path | What it is |
| --- | --- |
| `plugin/cflx-auto-resume.mjs` | OpenCode plugin. Filters the cflx enqueue tool, registers the sink, and runs the owner-continuity observer. |
| `callback/cflx-resume-session.mjs` | The argv Conflux executes. Reads the event file and posts one marked message. |
| `lib/cflx-mcp.mjs` | One-shot `cflx client mcp` client. |
| `lib/resume.mjs` | Message composition and the OpenCode POST. |
| `lib/loopback.mjs` | The destination policy. |

Requires Node 18+ (for `fetch`) and a `cflx` on `PATH` — or `CFLX_BIN` pointing at
one.

## Setup

1. Register `cflx client mcp` as an MCP server in your OpenCode config, so the
   agent gets `cflx_status`, `cflx_enqueue`, `cflx_wait`, and the notify tools.
2. Load `plugin/cflx-auto-resume.mjs` as an OpenCode plugin.
3. Point it at your local OpenCode server:

```bash
export OPENCODE_SERVER=http://127.0.0.1:4096   # must be loopback
export CFLX_BIN=/usr/local/bin/cflx            # optional
export CFLX_UNIX_SOCKET=/path/to/cflx-api.sock # optional; defaults to the repo's
export CFLX_AUTH_TOKEN_ENV=CFLX_TOKEN          # optional; a variable *name*
export CFLX_RESUME_STATE=/tmp/cflx-auto-resume # optional dedupe state directory
```

`CFLX_AUTH_TOKEN_ENV` names an environment variable that holds the token. A token
*value* is never accepted in argv, never printed, and never reaches the callback.

## The message is an ordinary user message

OpenCode stores what this posts as a `role=user` message. There is no trusted
internal event channel: to the model, the generated text is indistinguishable
from something you typed. That is why every generated message opens with

```text
[AUTOMATION EVENT — not user-authored]
```

and says explicitly that the event fields are data to verify against the
repository, not instructions to follow. **Treat event files and logs as untrusted
input.** If you change `lib/resume.mjs`, keep the marker: `resumeSession` refuses
to post a message without it.

## What the owner guarantees, and what it does not

**Does.** Exactly one terminal delivery per execution — `completed`, `failed`, or
`stopped`. `completed` is certified from current repository evidence for the
owner's declared terminal mode, the same oracle `cflx client wait` uses. A change
disappearing from the owner's snapshot is never completion. Registering *after*
the execution already settled delivers that terminal event immediately, so losing
the race between enqueue and registration does not lose the notification.

**Does not.** Survive a crash. Execution IDs and registrations are process-local
by design — under `openspec/CONSTITUTION.md` no out-of-worktree durable state may
influence workflow routing — so a killed owner takes its registrations with it. A
*graceful* stop attempts one `owner_stopping` event first; a `kill -9` cannot.

That is the one gap the plugin covers itself, with a low-frequency bounded
owner-continuity observer: if the owner vanishes or comes back as a different
incarnation, it resumes the session with a typed `owner_restarted` message. It
never reports that as success — nothing about the change was observed.

## Why a one-shot event file, not the lifecycle adapter stream

Conflux already has an optional **process lifecycle adapter**: a long-lived child
holding a stdin stream of semantic *process* transitions (`idle`, `working`,
`blocked`, `stopping`). It is the wrong shape for this.

A lifecycle adapter describes the process. A resident TUI going `idle` says the
process has nothing running — not that *your* proposal finished, and not which of
several admitted changes it was. Completion sinks are scoped to one admitted
execution, are proven from repository evidence rather than from presentation, and
fire once. Two integrations, two questions; configuring or delivering one never
requires the other.

The delivery mechanism follows from that. A one-shot argv plus an immutable event
file needs no long-lived child, no stream framing, and no reconnection logic, and
the file is removed once the callback is reaped.

## The callback contract

Conflux runs the registered argv **directly** — no shell, no `sh -c`, no quoting,
no expansion. The environment is *replaced*, not extended: the callback receives
exactly

```text
CFLX_EVENT_PATH     path to the versioned event file
CFLX_EVENT_TYPE     completed | failed | stopped | blocked | owner_stopping
CFLX_EXECUTION_ID   the execution episode
CFLX_CHANGE_ID      the change it belongs to
CFLX_INSTANCE_ID    the owner incarnation
```

and nothing else — no `PATH`, no `HOME`, no owner configuration, no credentials.
That is why the registered argv names the interpreter explicitly.

The event file holds bounded typed data only. It is immutable while the callback
runs and removed once the callback is reaped. Callback runtime and captured
output are bounded, and delivery failure is observability-only: a callback that
crashes, hangs, or exits non-zero cannot roll back, retry, or re-classify
anything.

Sink registration is accepted **only over the owner's Unix socket**. It stores an
argv the owner will execute, so an authenticated TCP client is refused with
`transport_not_permitted` — the credentials were fine, the channel was not.

## Reading the result

Branch on `outcome`, never on prose. Success is narrow:

| outcome | meaning |
| --- | --- |
| `admitted` / `already_admitted` | the owner accepted the intent. **Not completion.** |
| `subscribed` | a sink was set, read, or cleared |
| `completed` | repository evidence proved the owner's terminal mode |
| `owner_restarted` | the binding is gone. Not success, not failure — re-read the repository |
| `execution_not_found` | no such episode in this incarnation |
| `transport_not_permitted` | sink mutation was attempted off the Unix socket |
| `incompatible_owner` | this owner does not serve execution sinks at all |

## Packaging

This directory is repository-distributed and deliberately **outside** the crate's
`include` list in `Cargo.toml`, so `cargo package` does not ship it. It is
reference material for operators, not a dependency of the binary.
