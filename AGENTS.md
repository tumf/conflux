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
cflx client mark add-my-change --json     # select it; unrelated marks are preserved
cflx client unmark add-my-change --json
cflx client start --json                  # F5 equivalent over the authoritative marks
cflx client stop --json                   # graceful; force-stop for the immediate one
cflx client force-stop-change add-my-change --json   # kill exactly this proposal, now
cflx client wait add-my-change --json     # waits for as long as the work takes
cflx client wait add-my-change --timeout 45m --json   # or give up at an explicit deadline
I=$(cflx client status --json | jq -r .instance_id)
cflx client subscribe set add-my-change --instance-id "$I" --json -- /absolute/callback --flag v
cflx client subscribe get add-my-change --instance-id "$I" --json
cflx client subscribe clear add-my-change --instance-id "$I" --json
cflx client mcp                           # serve the same controls over stdio MCP

# Another project, from anywhere: name the project, not its socket.
cflx client --project-dir /absolute/path/to/repo status --json
```

The verbs are the operator's own — `status`, `mark`, `unmark`, `start`, `stop`,
`force-stop`, `force-stop-change`, `wait`, and the `subscribe` group — plus `mcp`
for hosts that speak the protocol instead. Connection options belong to the
namespace.
`--project-dir ABSOLUTE_PATH` is the normal explicit route: it names any
directory inside the project's Git working tree — the root, a subdirectory, a
linked worktree, a submodule — and Conflux derives *both* the owner socket
(`<git-common-dir>/cflx-api.sock`) and the repository that certifies completion
from that one project, so `wait` can never pair one project's owner with another
project's evidence. `--unix-socket PATH` is the low-level override for
diagnostics, tests, and owners that are not reachable through a repository; it
overrides the same default. The two conflict at parse time, and
`--auth-token-env NAME` names an environment variable holding the bearer token —
a token value is never accepted in argv and never printed. With neither route
option, the current working directory's repository is used.

**The three ideas are kept apart on purpose.**

- *Execution mark*: operator selection. `mark` and `unmark` name 1 through 64
  distinct proposals, set exactly those marks, and preserve every unrelated one.
  They submit only the shared `SetExecutionMark` intent — never queue intent,
  Start, or Retry — create no execution ID, and return once the commands settle
  without waiting for admission. A target already in the requested state settles
  as a reasoned `unchanged` no-op, and so does a terminal row, with the shared
  service's own stable reason.
- *Start*: explicit lifecycle control. `start` submits the same intent as TUI
  F5/`!` and consumes the owner's authoritative mark set; there is no target
  list, because "start only these" is not something the shared transaction can
  express. `stop` and `force-stop` are the shared stop controls with the same
  runtime classification the TUI applies.
- *Targeted force-stop*: the one control that is lifecycle *and* target-scoped.
  `force-stop-change` names exactly one proposal — never zero, never a list — and
  is the only path that bypasses the graceful SIGTERM escalation window. It is
  not a spelling of `force-stop`, and neither verb can be widened into the other
  by adding or dropping an argument.
- *Queue intent*: owner-side admission state, produced by the owner's own mark
  settlement and analysis. No client path constructs it.

**Read `outcome`, not prose.** `--json` prints exactly one versioned envelope on
stdout (`schema_version`, `ok`, `operation`, `outcome`, `instance_id`,
`execution_id`, `change_id`, `message`, `detail`); diagnostics go to stderr, and
each outcome has its own stable exit status. Operations are `status`,
`control_mark`, `control_unmark`, `control_start`, `control_stop`,
`control_force_stop`, `control_force_stop_change`, `wait`, `subscribe_set`,
`subscribe_get`, and `subscribe_clear`. Success is narrow: `observed`, `marked`,
`unmarked`, `unchanged`, `stopped`, `accepted`, `subscribed`, `cleared`,
`completed`. Everything else —
`owner_not_running`, `owner_not_command_capable`, `owner_restarted`,
`change_not_found`, `target_ineligible`, `revision_conflict`,
`transport_not_permitted`, `unsupported_owner`, `partial_intent`,
`observation_conflict`, `evidence_error`, `change_rejected`,
`change_requires_action`, `process_failed`, `timeout`, `usage_error` — is a
non-zero refusal.

A multi-target request names no single `change_id`; `detail.targets` lists each
proposal with whether it changed and why. A mark result never carries an
`execution_id`, because a mark creates no episode.

**Prerequisites.** An owner has to be running and command-capable. A headless
`cflx run` serves every read resource but binds no command executor, so `mark`
and `start` against it return `owner_not_command_capable` rather than queueing
for later; `status` and `wait` still work there. `wait` needs the owner's Git
repository, because it certifies completion from repository evidence: run it
inside that repository, or name the repository with `--project-dir`.

**Targeted force-stop kills one proposal and nothing else.** `cflx client
force-stop-change <change-id>` submits the shared `force_stop_change` command:
the owner sends SIGKILL straight to the managed process group that proposal owns
— no SIGTERM, no grace window, which is the whole difference from
`stop_and_dequeue` — waits for confirmed termination and reaping, then clears
that change's queue admission and execution mark atomically so later mark
settlement cannot redispatch it. Unrelated changes keep their processes, marks,
queue intent, execution IDs, and subscriptions; `app_mode`, scheduler state, and
process-wide stop state do not change. Completed worktree effects are preserved
and the settled result publishes `effects_rolled_back: false` alongside the
target, its cancelled `execution_id` when one existed, the cancelled phase, the
last completed phase, and whether a process was really terminated.

The killed proposal settles into the terminal `stopped` row, not into idle `not
queued` work, so a concurrent `cflx client wait` on it is released with
`change_requires_action` instead of holding out for an owner that will never
advance it again.

The success outcome is `stopped`, with exit status `0`. Eligibility is the
owner's own published fact, readable ahead of time as
`actions.force_stop_change`: an applying, accepting, rejecting, archiving, or
resolving target is eligible while it owns live managed activity; an admitted but
idle `queued` or `blocked` target is eligible for dequeue-only settlement; and
merge-wait, resolve-wait, terminal, rejected, unknown, and unadmitted rows are
refused with `target_ineligible` and a typed reason. Process-wide `force_stop`
and graceful `stop_and_dequeue` are unchanged.

**Marking admits nothing.** A settled mark proves the owner recorded your
selection, and nothing else. The owner's own settlement decides whether stable
marked work is admitted; a 10-second stability window governs that, so a mark
followed immediately by `start` is the explicit path, not the implicit one.
`wait` is the observation-only counterpart: it submits no start, retry, queue,
resolve, archive, merge, or cleanup command, and returns `completed` only when
current Git/OpenSpec evidence proves the owner's declared terminal mode
(`merged`, `base_published`, or `branch_pushed`) was reached. A change
disappearing from the snapshot is never completion.

**`wait` waits for the owner, not for you.** It keeps observing exactly the rows
this owner can still advance by itself — `not queued`, `queued`, `blocked`,
`applying`, `accepting`, `rejecting`, `archiving`, `resolving` — and releases the
caller as soon as the row is one only a new operator action can move. `error`,
`merge wait`, `stopped`, and `stalled` return `change_requires_action` with exit
status `27`, carrying `detail.observed_status`, `detail.error_detail` when the
owner published one, and `detail.commands_submitted: 0`; `rejected` keeps its own
`change_rejected`. `blocked` is the one row the display status cannot classify
alone, so its structured blocker decides: a dependency wait — or a hold with no
structured blocker at all — is work this owner clears by itself and keeps the
wait observing, while blocker kind `external` is a validated non-repository
prerequisite the owner already handed back to you. That releases with the same
`change_requires_action`, and `detail.blocker` carries the blocker's own
`unblock_condition` and `prerequisite_owner` so the released caller knows what to
satisfy rather than parsing a message. Releasing is still not repairing: no
start, retry, queue, resolve, archive, merge, or cleanup command is submitted on
the way out, and what to do about a parked change remains the operator's
decision. The classification
runs on the first observation and on every later coherent one, so waiting on a
change that was already parked before you asked reports it immediately instead of
hanging until your deadline. A settled `merged`, `pushed`, or `archived` row is
still only a claim: it is certified against the repository, gets one bounded
re-observation and re-certification for evidence that landed just after the row
moved, and then returns either `completed` or the same `change_requires_action`
rather than holding forever on a verdict nobody will revise. Script the new
outcome: an automatically progressing wait behaves exactly as before, but a
change that needs a human now says so instead of never returning.

**`wait` has no deadline unless you ask for one.** `--timeout` defaults to `0`,
and `0` in any unit — `0`, `0s`, `0ms`, `0m`, `0h` — means "wait for as long as
the work takes": the wait ends at verified completion, a typed failure, owner
replacement, or your own cancellation, and never because a clock ran out. Pass a
positive `--timeout D` when you want the opposite, and its expiry is the typed
`timeout` outcome. A positive value below `100ms` or above `7d` is still a usage
error. Either way the bound on the *operation* is not a bound on its
subprocesses: each owner request keeps the transport's own per-request valve, and
each Git child an unbounded wait spawns keeps a finite per-invocation deadline
whose expiry terminates and reaps that child and retries the check — it is never
reported as the operation-level `timeout`.

If a later target fails after earlier commands settled, the request returns
`partial_intent` listing exactly the command records it created, in order. It
never claims a rollback: undoing a settled mark would be a mark mutation racing
whoever set it.

`/api/v2` remains the lower-level generated contract for anything these
commands do not cover; prefer `cflx client` for delegation.

### `cflx client mcp`

`cflx client mcp` serves the same boundary to an MCP host over stdio, as three
closed tools: `cflx_status`, `cflx_control`, and `cflx_subscribe`.
`cflx_control` takes one `action` — `mark`, `unmark`, `start`, `stop`,
`force_stop`, or `force_stop_change` — with `change_ids` required by the two mark
actions (1 through 64, distinct), required as exactly one element by
`force_stop_change`, and refused by the three process-wide lifecycle ones. Zero,
several, duplicate, or blank targets on `force_stop_change` are refused through
the normal validation error before any owner is contacted. `cflx_subscribe` takes `set`, `get`, or `clear` over 1
through 64 distinct `change_ids`.

Every tool accepts an optional absolute `project_dir` — the normal per-call
selector — and an optional `unix_socket` low-level override, so **register the
server once, globally, with no route option at all** and let each call name its
project:

```json
{"name": "cflx_control",
 "arguments": {"action": "mark", "change_ids": ["add-my-change"],
               "project_dir": "/absolute/path/to/repo"}}
```

One server process drives any number of projects that way, and nothing is
remembered between calls: a call-scoped selector shadows the namespace default
rather than writing to it, so two concurrent calls cannot move each other's
route. `project_dir` and `unix_socket` in the *same* call are refused through
the normal MCP validation error before any owner is contacted — no new envelope
outcome — and so is a relative path, a bare repository, or a directory that is
not a usable Git working tree.

It is still a client — no lock, no listener, no run — and it exposes no raw
command construction, so a model cannot name a command type, an expected
revision, an idempotency key, queue intent, or shell source. stdout carries
JSON-RPC frames and nothing else; diagnostics go to stderr.

`cflx_wait` is deliberately absent from MCP. A completion wait stays open for
as long as the work takes, which is not the shape of a tool call. `cflx client
wait` remains the CLI completion oracle, and an MCP host that wants asynchronous
completion registers an explicit callback with `cflx_subscribe`. A host that
cannot execute callback argv has no MCP completion oracle, by design.

### Completion notifications

**Nothing is registered for you.** There is no post-tool hook, no inference from
a control result, and no automatic agent or session resume. An agent that wants
to be told when a proposal finishes asks explicitly — a shell through `cflx
client subscribe`, an MCP host through `cflx_subscribe`. Neither requires the
other.

```bash
I=$(cflx client status --json | jq -r .instance_id)
cflx client subscribe set alpha beta --instance-id "$I" --json -- /absolute/callback --flag v
cflx client subscribe get alpha --instance-id "$I" --json
cflx client subscribe clear alpha beta --instance-id "$I" --json

# The same three against another project, routed by directory rather than socket:
P=/absolute/path/to/repo
I=$(cflx client --project-dir "$P" status --json | jq -r .instance_id)
cflx client --project-dir "$P" subscribe set alpha --instance-id "$I" --json -- /absolute/callback
```

A subscription is keyed by the **proposal**, not by an execution episode, so it
can be registered before the owner admits anything — the gap an execution-scoped
registration could not cover. Each operation names 1 through 64 distinct
proposals and the owner incarnation it expects, so a replaced owner is reported
as typed `owner_restarted` rather than silently registering against a process
that never saw your work. `--blocked` opts into the non-terminal attention edge.
Everything after `--` is the callback argv — one element per argument, exactly as
typed — and the CLI never parses shell source. The envelopes are the group's own:
`subscribe_set`, `subscribe_get`, `subscribe_clear`, with `subscribed` and
`cleared` as the success tokens.

Whenever a subscribed proposal enters a new execution episode, the owner binds
that episode and runs the argv **once** when it reaches a typed terminal
classification — `completed`, `failed`, or `stopped` — with `blocked` as an
opt-in attention edge and `owner_stopping` on graceful shutdown only. **This is
execution completion, not process completion.** The TUI stays alive after work
finishes, so process exit was never a signal; and a lifecycle adapter's `idle`
describes the process, not your proposal. `completed` uses the same repository
oracle `wait` certifies with, so a change disappearing from the snapshot is never
completion.

Constraints worth knowing before you wire one up:

- **Delivery notifies; it does not resume.** Conflux executes the registered argv
  and draws no conclusion from it. It starts no agent, resumes no session, infers
  no messaging destination, and a callback that fails, hangs, or exits non-zero
  cannot roll back, retry, or re-classify anything. Anything an agent does next
  is external and explicit, and it must revalidate the current owner and
  repository evidence rather than trusting the event text.
- **Dedupe is keyed by the episode.** Replacing a subscription, clearing it, or
  clearing and setting it again never replays a terminal event this owner already
  delivered. Registering *after* the latest episode settled delivers that event
  immediately, once — which is what stops a start/registration race from losing a
  notification. Re-admission after a retry is a new episode and a new delivery.
- **UDS only for mutation, and for argv.** A subscription stores an argv the
  owner will execute, so `set` and `clear` are accepted only over the owner's
  Unix socket; an authenticated TCP client is refused with
  `transport_not_permitted`. Reads work on either transport, but the *registered
  argv* comes back only over that same socket: a channel that may not register a
  command may not read one back. Presence, execution state, and delivery history
  are answered on both. Every request, inspection included, carries the complete
  `(instance_id, change_id)` binding.
- **argv, not shell.** No `sh -c`, no quoting, no expansion. The environment is
  *replaced* with exactly `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`,
  `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID` — no owner token,
  no configuration, no inherited `PATH`. `CFLX_CHANGE_ID` keeps its name: the
  wire identifier is still `change_id`, and "proposal" is the word for humans.
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
- **Nothing survives a crash.** Subscriptions and execution IDs are
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

Why a one-shot event file rather than the lifecycle adapter's long-lived stdin
stream: the adapter answers "what is this *process* doing", stays attached, and
is framed as a session. A completion subscription answers "did *this execution
episode of my proposal* finish", is proven from repository evidence rather than
presentation, and fires once — so it needs no long-lived child, no stream
framing, and no reconnection. Configuring or delivering either never requires the
other.

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
- `PUT`/`DELETE /api/v2/executions/{execution_id}/sink` and
  `PUT`/`DELETE /api/v2/proposals/{change_id}/subscription` are accepted only
  here. They store an argv this process will execute, so TCP is refused with
  `transport_not_permitted` even when bearer authentication succeeds. `GET` is
  served on both listeners, but it returns the registered argv only here;
  elsewhere it answers presence (`sink_registered` / `subscribed`), execution
  state, and delivery history with `sink: null`. The execution-scoped resource
  requires the complete `(instance_id, execution_id, change_id)` binding; the
  proposal-scoped one requires `(instance_id, change_id)`, with the change in the
  path. `GET /api/v2/capabilities` reports whether each surface exists at all.
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
