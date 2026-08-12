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
cflx client enqueue add-my-change --json  # ask the owner to admit one change
cflx client wait add-my-change --timeout 45m --json
```

Three commands, and only three. Connection options belong to the namespace:
`--unix-socket PATH` overrides the default `${GIT_COMMON_DIR}/cflx-api.sock`, and
`--auth-token-env NAME` names an environment variable holding the bearer token —
a token value is never accepted in argv and never printed.

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
later; `status` and `wait` still work there. `wait` must run inside the owner's
Git repository, because it certifies completion from repository evidence.

**`enqueue` admits; it does not complete.** A successful enqueue proves the owner
accepted the intent, nothing more. `wait` is the observation-only counterpart: it
submits no start, retry, queue, resolve, archive, merge, or cleanup command, and
returns `completed` only when current Git/OpenSpec evidence proves the owner's
declared terminal mode (`merged`, `base_published`, or `branch_pushed`) was
reached. A change disappearing from the snapshot is never completion. If an
idle-owner start would consume execution marks that are not yours, `enqueue`
refuses with `operator_intent_conflict` and leaves them untouched.

`/api/v2` remains the lower-level generated contract for anything the three
commands do not cover; prefer `cflx client` for delegation.

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
