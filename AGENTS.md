# AGENTS.md - Conflux

Essential information for AI coding agents working on this Rust codebase.

## Project Overview

Conflux(cflx) automates the OpenSpec change workflow (list → dependency analysis → apply → acceptance → archive → resolve → merged). It orchestrates `openspec` and AI coding agent tools to process changes autonomously.

## Self-hosted Development

* Find cflx logs: `~/.local/state/cflx/logs/conflux-{slug}/YYYY-MM-DD.log`

## Frontends
Conflux has the following frontends:

* TUI
* WebUI (local `--web` monitoring)

## Delegating to an existing owner

`cflx client` is how an agent hands work to the Conflux process that already
owns this repository. It is a **client**, not an owner: it never takes the
orchestration lock, binds a listener, starts a run, launches a lifecycle adapter
or an AI subprocess, or writes to the workspace. That is the whole difference
from `cflx run`, which *is* an owner of a finite explicit-target run and will
contend for the lock with the process you meant to talk to.

```bash
cflx client status --json                 # read the owner; mutates nothing
EXEC=$(cflx client enqueue add-my-change --json | jq -r '.execution_id')
cflx client wait add-my-change --timeout 45m --json
cflx client notify set add-my-change "$EXEC" --json -- /absolute/callback --flag v
cflx client notify get add-my-change "$EXEC" --json
cflx client notify clear add-my-change "$EXEC" --json
cflx client mcp                           # serve the same intents over stdio MCP

# Another project, from anywhere: name the project, not its socket.
cflx client --project-dir /absolute/path/to/repo status --json
```

Four verbs, and only four: `status`, `enqueue`, `wait`, and the `notify` group,
plus `mcp` for hosts that speak the protocol instead. Connection options belong
to the namespace. `--project-dir ABSOLUTE_PATH` is the normal explicit route:
it names any directory inside the project's Git working tree — the root, a
subdirectory, a linked worktree, a submodule — and Conflux derives *both* the
owner socket (`<git-common-dir>/cflx-api.sock`) and the repository that
certifies completion from that one project, so `wait` can never pair one
project's owner with another project's evidence. `--unix-socket PATH` is the
low-level override for diagnostics, tests, and owners that are not reachable
through a repository; it overrides the same default. The two conflict at parse
time, and `--auth-token-env NAME` names an environment variable holding the
bearer token — a token value is never accepted in argv and never printed. With
neither route option, the current working directory's repository is used,
exactly as before.

**It is intent-based on purpose.** The CLI reads authoritative capabilities,
instance identity, state, execution status, and per-change action eligibility,
then picks the route itself: retry for a change carrying retryable evidence,
queue intent for a live scheduler, an isolated execution mark plus start for an
idle owner. You never construct a revision, a command type, a queue mark, or an
idempotency key — the client owns revision refresh, command-record settlement,
and bounded stale-revision retries.

**Read `outcome`, not prose.** `--json` prints exactly one versioned envelope on
stdout (`schema_version`, `ok`, `operation`, `outcome`, `instance_id`,
`change_id`, `message`, `detail`); diagnostics go to stderr, and each outcome has
its own stable exit status. Success is narrow: `observed`, `admitted`,
`already_admitted`, `completed`. Everything else — `owner_not_running`,
`owner_not_command_capable`, `owner_restarted`, `change_not_found`,
`target_ineligible`, `operator_intent_conflict`, `partial_intent`,
`revision_conflict`, `observation_conflict`, `evidence_error`, `change_rejected`,
`process_failed`, `timeout` — is a non-zero refusal that started nothing.

**Prerequisites.** An owner has to be running and command-capable. A headless
`cflx run` serves every read resource but binds no command executor, so
`enqueue` against it returns `owner_not_command_capable` rather than queueing for
later; `status` and `wait` still work there. `wait` needs the owner's Git
repository, because it certifies completion from repository evidence: run it
inside that repository, or name the repository with `--project-dir`.

**`enqueue` admits; it does not complete.** A successful enqueue proves the owner
accepted the intent, nothing more. `wait` is the observation-only counterpart: it
submits no start, retry, queue, resolve, archive, merge, or cleanup command, and
returns `completed` only when current Git/OpenSpec evidence proves the owner's
declared terminal mode (`merged`, `base_published`, or `branch_pushed`) was
reached. A change disappearing from the snapshot is never completion. If an
idle-owner start would consume execution marks that are not yours, `enqueue`
refuses with `operator_intent_conflict` and leaves them untouched.

`/api/v2` remains the lower-level generated contract for anything these
commands do not cover; prefer `cflx client` for delegation.

### `cflx client mcp`

`cflx client mcp` serves the same boundary to an MCP host over stdio, as six
closed tools: `cflx_status`, `cflx_enqueue`, `cflx_wait`, and `cflx_notify_set` /
`_get` / `_clear`. Every one of them accepts an optional absolute `project_dir`
— the normal per-call selector — and an optional `unix_socket` low-level
override, so **register the server once, globally, with no route option at all**
and let each call name its project:

```json
{"name": "cflx_enqueue",
 "arguments": {"change_id": "add-my-change", "project_dir": "/absolute/path/to/repo"}}
```

One server process drives any number of projects that way, and nothing is
remembered between calls: a call-scoped selector shadows the namespace default
rather than writing to it, so two concurrent calls cannot move each other's
route. `project_dir` and `unix_socket` in the *same* call are refused through
the normal MCP validation error before any owner is contacted — no new envelope
outcome — and so is a relative path, a bare repository, or a directory that is
not a usable Git working tree. `cflx_wait` certifies completion from the
selected project's repository only, never from the server process's own.

It is the alternative for a host that speaks the protocol
rather than a shell, and it calls exactly the modules the commands do. It is
still a client — no lock, no listener, no run — and it exposes no raw command
construction, so a model cannot name a command type, an expected revision, an
idempotency key, an execution mark, or shell source. stdout carries JSON-RPC
frames and nothing else; diagnostics go to stderr.

`cflx_enqueue` settles and returns. Its `execution_id` names one *admitted
execution episode*, not the change: a retry of the same proposal gets a new ID,
and iterations inside one admitted run keep theirs. Concurrent callers that find
the change already admitted observe the same current ID.

### Completion notifications

**If you have a shell, use `cflx client notify`.** It is the direct adapter over
the same execution-scoped completion sink, so nothing about it requires an MCP
host:

```bash
EXEC=$(cflx client enqueue add-my-change --json | jq -r .execution_id)
cflx client notify set add-my-change "$EXEC" --json -- /absolute/callback --flag v
cflx client notify get add-my-change "$EXEC" --json
cflx client notify clear add-my-change "$EXEC" --json

# The same four against another project, routed by directory rather than socket:
P=/absolute/path/to/repo
EXEC=$(cflx client --project-dir "$P" enqueue add-my-change --json | jq -r .execution_id)
cflx client --project-dir "$P" notify set add-my-change "$EXEC" --json -- /absolute/callback
```

Each operation names an *admitted execution episode*, not a change: pass the
`execution_id` the enqueue reported, and pass `--instance-id` too when you kept
it, so a replaced owner is reported as typed `owner_restarted` instead of the
`execution_not_found` a new incarnation would otherwise answer with. `--blocked`
opts into the non-terminal attention edge. Everything after `--` is the callback
argv — one element per argument, exactly as typed — and the CLI never parses
shell source. The envelopes are the namespace's own: `notify_set`, `notify_get`,
`notify_clear`, with `subscribed` as the single success token.

`cflx client notify set`, and `cflx_notify_set` behind it, attach one bounded
argv the owner runs **once** when that execution reaches a typed terminal
classification — `completed`, `failed`, or `stopped` — with `blocked` as an
opt-in attention edge and `owner_stopping` on graceful shutdown only. **This is
execution completion, not process completion.** The TUI stays alive after work
finishes, so process exit was never a signal; and a lifecycle adapter's `idle`
describes the process, not your proposal.
`completed` uses the same repository oracle `wait` certifies with, so a change
disappearing from the snapshot is never completion. Registering *after* the
execution settled delivers that terminal event immediately, which is what stops
the enqueue/registration race from losing a notification.

Constraints worth knowing before you wire one up:

- **UDS only for mutation, and for argv.** A sink stores an argv the owner will
  execute, so `set` and `clear` are accepted only over the owner's Unix socket;
  an authenticated TCP client is refused with `transport_not_permitted`. Reads
  work on either transport, but the *registered argv* comes back only over that
  same socket: a channel that may not register a command may not read one back.
  Everything else a caller needs — subscription presence, execution state,
  delivery history — is answered on both. Every request, inspection included,
  carries the complete `(instance_id, execution_id, change_id)` binding; a
  partial one is refused, because a coherence check that accepted half a binding
  would let you and the owner mean two different episodes.
- **argv, not shell.** No `sh -c`, no quoting, no expansion. The environment is
  *replaced* with exactly `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`,
  `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID` — no owner token,
  no configuration, no inherited `PATH`.
- **A token is a variable name.** `--auth-token-env NAME`. Token values never
  appear in argv, in a tool result, in a log, or in an event file.
- **The event file is read-only by default, and never read back.** The payload is
  created `0400` inside a `0700` owner-private directory, so opening
  `CFLX_EVENT_PATH` for writing is refused. That is default mutation refusal, not
  an integrity guarantee: your callback runs under the owner's UID and can
  `chmod` past it. What makes that harmless is that the owner writes the file
  once and never reads it back, so editing it changes no delivered
  classification. It is removed once your callback is reaped.
- **Output is bounded but never blocked.** stdout and stderr are drained for the
  whole life of the callback; past the retained ceiling the excess is read and
  discarded, so a chatty callback never wedges on a full pipe and never grows the
  owner. Overflow is a truncation diagnostic, not a delivery failure, and it does
  not terminate the callback. Only the runtime ceiling and shutdown cancellation
  do, and both kill *and* reap.
- **Nothing survives a crash.** Execution IDs and registrations are
  process-local, as the constitution requires; a `kill -9` takes them with it. A
  *graceful* stop attempts one `owner_stopping` first. Delivery stays serialized
  through shutdown under a single finite deadline covering every queued or
  running callback: past it nothing new starts, no event artifact is created, and
  the callback still running is terminated and reaped *before* any artifact is
  removed. An external adapter that needs to cover the crash keeps its own
  bounded owner-continuity check and reports typed `owner_restarted` — never
  success.
- **Only a confirmed reap removes anything.** Cleanup waits for the dispatcher's
  reap acknowledgement, and that acknowledgement is the *sole* authority: the
  owner-private directory is an owned path, not a temporary-directory handle, so
  dropping the registry deletes nothing. If the acknowledgement never arrives —
  the dispatcher was already gone, or it dropped the acknowledgement instead of
  sending it — the directory and its artifacts are **retained** and their path is
  logged, because an unacknowledged reap says nothing about whether a callback
  child is still holding the file. A retained directory is recoverable; a payload
  pulled out from under a live callback is not. Nothing cleans those retained
  files up for you.
- **Delivery is observability.** A callback that fails, hangs, or exits non-zero
  cannot roll back, retry, or re-classify anything.

Why a one-shot event file rather than the lifecycle adapter's long-lived stdin
stream: the adapter answers "what is this *process* doing", stays attached, and
is framed as a session. A completion sink answers "did *this admitted execution*
finish", is proven from repository evidence rather than presentation, and fires
once — so it needs no long-lived child, no stream framing, and no reconnection.
Configuring or delivering either never requires the other.

`examples/integrations/hermes-auto-resume/` wires this into Hermes: its
`post_tool_call` hook preserves the qualifying enqueue call's own `project_dir`
(or its low-level `unix_socket`) and registers through
`cflx client --project-dir … notify set`, so one gateway process can serve
several projects. A call that named no route at all fails closed rather than
guessing from `CFLX_UNIX_SOCKET`, which remains a compatibility fallback only
for hosts that expose no tool arguments at all.

`examples/integrations/opencode-auto-resume/` wires this into OpenCode. Its
generated messages are ordinary OpenCode `role=user` messages — there is no
trusted internal event channel — so every one opens with
`[AUTOMATION EVENT — not user-authored]`, and event files and logs are treated as
untrusted data, never as instructions.

## Local API socket

`cflx`, `cflx tui`, and `cflx run` serve the versioned `/api/v2` API on
`${GIT_COMMON_DIR}/cflx-api.sock` by default in `web-monitoring` builds — no
TCP port and no flag required. Linked worktrees of one repository share that
single socket because it is derived from the same canonical Git common directory
the repository lock uses, and the lock is what prevents two default owners.

```bash
curl --unix-socket "$(git rev-parse --git-common-dir)/cflx-api.sock" \
  http://localhost/api/v2/state
```

- `--web-unix-socket PATH` overrides the path; `--no-web-unix-socket` disables
  the listener. The two are mutually exclusive.
- Outside a Git repository the default has no identity to derive, so startup
  fails unless one of those two options is supplied.
- The socket is mode `0600`. A configured bearer token applies to UDS and TCP
  alike (`/api/v2/health` stays public); without one, UDS is token-free local
  access, protected by filesystem permissions.
- `PUT`/`DELETE /api/v2/executions/{execution_id}/sink` are accepted only here.
  They store an argv this process will execute, so TCP is refused with
  `transport_not_permitted` even when bearer authentication succeeds. `GET` is
  served on both listeners, but it returns the registered argv only here;
  elsewhere it answers `sink_registered`, execution state, and delivery history
  with `sink: null`. All three require the complete
  `(instance_id, execution_id, change_id)` binding in the query or body.
- The listener must be bound before lifecycle adapters, AI subprocesses, or
  orchestration start. A bind, permission, or path-safety failure exits non-zero
  with nothing started; a finite run removes its own socket on completion.
- A live socket or non-socket entry at the target path is never removed; only an
  unreachable stale socket is replaced.
- Browsers cannot open a `unix://` endpoint. It is for local clients and reverse
  proxies; the QR popup still encodes the TCP URL only.

### Before intervening in a live run

`GET /api/v2/execution-status` is the resource that answers "is anything
actually running". `scheduler_running` and `has_active_work` are separate: a
parked persistent scheduler is alive with nothing admitted.

`cflx run` serves this resource too, and its lifecycle work shows up in
`has_active_work`, `active_activities`, and the per-change phases. It binds no
run supervisor and no command executor, though, so `scheduler_running` stays
false for a whole run and `/api/v2` commands are rejected there — read the work
fields, not scheduler liveness, to decide whether a run is busy.

Never infer that Apply produced no commit from `display_status: applying`, or
from a `stop_and_dequeue` that merely returned success — Apply can finish while
a cancellation is still in flight. Read the settled command record's typed
`result` instead: `cancelled_phase`, `last_completed_phase`, `apply_commit`
(where `present: null` means *unknown*, not absent), and `effects_rolled_back`,
which is always `false`. Creating a manual commit, archive, or merge on that
guess is exactly the failure this contract exists to prevent.

## Web UI

The WebUI is an optional local monitoring dashboard enabled with `--web` on
`cflx`, `cflx tui`, or `cflx run`. `--web` *adds* the TCP listener alongside the
Unix socket; it never replaces it, and both listeners serve the same router and
`WebState`. There is no standalone server daemon and no multi-project mode.

The operator console consists of `web/index.html`, `web/style.css`, and
`web/app.js`, embedded via `include_str!` in `src/web/mod.rs`. No build step and
no frontend framework: it is dependency-free HTML/CSS/JS and a first-class
`/api/v2` client. There is no legacy unversioned `/api/*` or `/ws` surface any
more. Browser tests live in `tests/web/` and run with `make web-test`; that
tooling is dev-only and must never become a production dependency.

## Directories

* `src/`: Main Rust source code
* `tests/`: Rust test code
* `web/`: Embedded static web assets used by the WebUI
* `skills/`: Source files for `cflx-*` skills that are embedded into the Rust binary
* `openspec/`: OpenSpec changes and specs
* `docs/`: Project documentation
* `scripts/`: Development and release helper scripts

## Serial or Parallel Mode

* Parallel mode: Mainly used
* Serial mode: Obsolete (to be removed)

## Constitution

* `openspec/CONSTITUTION.md` が存在する場合、proposal・spec・implementation より上位の規範として必ず従うこと。
* 憲法レベルの原則を変更する場合は、`openspec/CONSTITUTION.md` 自体を同じ change で明示的に更新すること。

## Skills

It also depends on `cflx-*` skills developed under the `skills/` directory.
The skill files are embedded into the Rust binary via `include_str!` at compile time.
**NEVER EDIT** `~/.agents/skills/cflx-*` skills. These will be overwritten by `cflx install-skills --global`.

## Unit Tests

Tests taking over 1 second must either be optimized to run in under 1 second or, if that is not practical, marked with `#[cfg_attr(not(feature = "heavy"), ignore)]`. Heavy tests must not run as part of the default test suite.

The one-second rule is a target for total test runtime, not permission to use short wall-clock thresholds as correctness assertions. Verify liveness, concurrency, and non-blocking behavior deterministically with event ordering, channels, barriers, or state transitions. Use timeouts only as generous safeguards against hangs, accounting for loaded CI and slow platforms; verify performance requirements in dedicated benchmarks.

Use `bd` for task tracking.
