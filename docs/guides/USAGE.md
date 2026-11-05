# Conflux Usage Examples

## Quick Start (Golden Path)

### 1. Install

```bash
cargo install cflx
```

### 2. Initialize Configuration

Generate a configuration file for your preferred AI agent:

```bash
# Default: Claude Code template
cflx init

# OpenCode template
cflx init --template opencode

# Codex template
cflx init --template codex

# Overwrite existing config
cflx init --force
```

For a full reference of `.cflx.jsonc` and `~/.config/cflx/config.jsonc`, see [CONFIG.md](./CONFIG.md).

### 3. Launch the TUI (Default)

The primary interface is the interactive TUI dashboard:

```bash
cflx
```

In the TUI:
- Use `Space` to select changes
- Press `F5` to start processing

### 4. Run Headless (Non-Interactive)

Process all pending changes without the TUI:

```bash
cflx run
```

The `cflx run` command will:
1. List pending changes via `openspec list`
2. Analyze dependencies and select the next change
3. Apply changes using the configured AI agent command
4. Run acceptance (if configured)
5. Archive completed changes
6. Repeat until all changes are processed

## Common Usage Patterns

### Process a Specific Change

```bash
cflx run --change add-feature-x
```

### Process Multiple Specific Changes

```bash
cflx run --change add-feature-x,fix-bug-y,refactor-z
```

### Use a Custom Configuration File

```bash
cflx run --config /path/to/config.jsonc
```

## Workspace Execution

Conflux uses Git worktrees in the default execution mode:

```bash
# Preview workspace grouping without executing
cflx run --dry-run

# Execute with the default workspace mode
cflx run
```

Resume behavior:

```bash
# Existing workspaces are reused automatically
cflx run

# Force a fresh start (discard existing workspaces)
cflx run --no-resume
```

## Deleting a Worktree from the TUI

`Tab` switches to the Worktrees view; `D` opens the delete confirmation for the
worktree under the cursor. A worktree with no branch to name — a detached
HEAD — cannot be confirmed against anything later and is refused up front.

**Ordinary deletion.** `Y` runs `.wt/teardown` and then removes the worktree;
`S` skips teardown and removes it. Both delete the directory including generated
and ignored contents. Neither grants permission to destroy work: if the shared
service observes uncommitted changes or commits ahead of base, it refuses and
the TUI opens a second confirmation instead of deleting.

**Discarding uncommitted changes.** The `Discard Uncommitted Changes`
confirmation names the branch, the path, and the teardown choice already made.
Only uppercase `X` proceeds — `Y`, `S`, and lowercase `x` are inert — and `N` or
`Esc` keeps everything. Tracked, staged, and reported untracked files are lost;
nothing is stashed, committed, or backed up.

**Discarding unmerged commits.** The `Discard Unmerged Commits` confirmation
appears when the branch carries commits base does not have. It names the path,
the branch, the exact commit the deletion is authorized from, and the teardown
choice. Uppercase `X` deletes the worktree *and* force-deletes the local branch;
the commits become unreachable and are not recoverable. Nothing is merged,
pushed, tagged, stashed, or backed up first.

**When both apply.** A worktree that is dirty *and* ahead gets one confirmation
that states both losses, and the single `X` authorizes both. There is no way to
grant one from a confirmation that named only the other, and skipping teardown
stays a separate decision from either.

**Partial results.** Removing the worktree and deleting the branch are distinct
outcomes. The branch is deleted only if its ref still points at the confirmed
commit, through an atomic compare-and-delete. If the ref moved or could not be
deleted, the worktree is still gone, the branch is kept, and the log says
`Partially deleted worktree: … was retained because …`. Every safety fact is
re-observed after teardown and immediately before removal; anything that changed
or could not be determined refuses and leaves both the worktree and the branch
in place.

**Remote clients cannot do any of this.** `/api/v2` and the Web UI are
fail-closed: teardown is mandatory, and a dirty or ahead worktree is reported as
undeletable with a reason. There is no force flag, no discard parameter, and no
remote equivalent of the `X` confirmation.

## Web Monitoring

Enable the web monitoring server alongside the TUI or headless run:

```bash
# TUI with web monitoring (OS auto-assigns port)
cflx --web

# Headless run with web monitoring
cflx run --web

# Custom port, plus a bearer token because the bind is not loopback
export CFLX_WEB_TOKEN="$(openssl rand -hex 32)"
cflx --web --web-port 9000 --web-bind 0.0.0.0 --web-auth-token-env CFLX_WEB_TOKEN
```

Access the dashboard at `http://localhost:<port>/` (port shown in startup log).

### Binding beyond loopback

A non-loopback `--web-bind` requires a bearer token, and the process refuses to
start without one. Supply it with either:

- `--web-auth-token-env VAR` — recommended; the value never appears in the
  process's command line.
- `--web-auth-token TOKEN` — a literal value, visible to anything that can
  inspect process arguments.

The two are mutually exclusive.

### Remote-control API (`/api/v2`)

Alongside the dashboard, a single running process exposes a versioned
remote-control contract for scripts and tooling:

```bash
BASE=http://localhost:9000/api/v2
AUTH="Authorization: Bearer $CFLX_WEB_TOKEN"

curl "$BASE/health"                          # always unauthenticated
curl -H "$AUTH" "$BASE/capabilities"         # commands, transports, limits
curl -H "$AUTH" "$BASE/state"                # coherent snapshot + revision
curl -H "$AUTH" "$BASE/execution-status"     # what is actually executing now

# Every command needs the revision it was decided against and a replay key.
curl -H "$AUTH" -H 'Content-Type: application/json' "$BASE/commands" -d '{
  "type": "set_queue_intent", "change_id": "my-change", "queued": true,
  "expected_revision": 12, "idempotency_key": "01J8Z...-retry-1"
}'

# Resume the ordered event stream from a cursor.
curl -N -H "$AUTH" "$BASE/events?after_sequence=41&instance_id=$INSTANCE_ID"
```

Notes for client authors:

- `instance_id`, `state_revision`, and `event_sequence` are valid only for one
  process incarnation. Compare `instance_id` before reusing a cursor; a restart
  invalidates it.
- Credentials go in the `Authorization` header only. Tokens in query strings or
  WebSocket subprotocols are rejected, so browser-native `EventSource` and
  `WebSocket` cannot be used against a protected `/api/v2`; browsers read
  `/api/v2/events` with `fetch()` response streaming. The embedded operator
  console does exactly that — `/api/v2` is its only contract, and the legacy
  unversioned `/api/*` and `/ws` routes have been removed.
- Cross-origin access is same-origin by default. A reverse proxy that changes
  the externally visible origin must declare it with `--web-allowed-origin`
  (repeatable, exact `scheme://host[:port]`); wildcards and forwarded headers
  are never honored.
- Errors carry a stable `error_code` to branch on — `stale_revision` and
  `idempotency_mismatch` are both HTTP 409 but mean different things.
- `GET /execution-status` answers "is anything actually running", which
  `/state` does not. `scheduler_running` and `has_active_work` are separate
  fields: a parked persistent scheduler is alive with nothing admitted. Every
  timestamp there is an absolute UTC RFC 3339 instant, including `observed_at`;
  compute relative time against that rather than your own clock.
- `cflx run` serves the same resource, and its work is visible in
  `has_active_work`, `active_activities`, and the per-change phases. It binds no
  run supervisor and no command executor, so `scheduler_running` stays false for
  the whole run and `/api/v2` commands are rejected: judge a run by the work
  fields, not by scheduler liveness.
- Never conclude that Apply produced no commit from `display_status: applying`
  or from a `stop_and_dequeue` that merely succeeded. Apply can finish while a
  cancellation is in flight. Read the settled record's typed `result`:
  `cancelled_phase`, `last_completed_phase`, `apply_commit` (whose `present:
  null` means *unknown*, not absent), and `effects_rolled_back`, which is always
  `false` because dequeue never undoes completed worktree work.
- Log content is available through `GET /logs`, `/events`, and `/ws` only. No
  response, schema, event, or command result discloses a log file path, a
  repository root, a workspace path used as a log locator, or a `file://` URL,
  and no endpoint accepts a host path or filename.

### Configured-command worktree sessions

The Worktrees view's `Enter` and `+` keys run the *configured*
`worktree_command`. The same two actions are available over `/api/v2`, and they
never take a command from the client:

```bash
# Run the configured command in an existing worktree (opaque ID from GET /worktrees).
curl -H "$AUTH" -H 'Content-Type: application/json' "$BASE/commands" -d '{
  "type": "run_worktree_command", "target": {"worktree_id": "<32 hex>"}, "params": {},
  "expected_revision": 12, "idempotency_key": "session-1"
}'

# Create a ws-session-* worktree exactly as `+` does, then run the command there.
curl -H "$AUTH" -H 'Content-Type: application/json' "$BASE/commands" -d '{
  "type": "create_session_worktree", "params": {},
  "expected_revision": 12, "idempotency_key": "session-2"
}'

curl -H "$AUTH" "$BASE/worktree-sessions"               # newest first
curl -H "$AUTH" "$BASE/worktree-sessions/$SESSION_ID"   # one session with its output tail

curl -H "$AUTH" -H 'Content-Type: application/json' "$BASE/commands" -d '{
  "type": "stop_worktree_session", "target": {"session_id": "'$SESSION_ID'"}, "params": {},
  "expected_revision": 12, "idempotency_key": "stop-1"
}'
```

- **Configured command only.** The command is the configured `worktree_command`
  template with `{workspace_dir}` and `{repo_root}` expanded by the owner. No
  request field carries a command, argv, environment, path, branch, or base
  commit; any such member is a 422 `validation_failed`. Without a configured
  template, outside a Git repository, or while the owner is shutting down,
  launches are refused `worktree_session_unavailable` before any Git or process
  effect. A headless `cflx run` binds no command executor, so it answers
  `command_executor_unbound`. `GET /capabilities` reports
  `worktree_sessions.available` and the limits below.
- **Same runner as the TUI.** `sh -c`, stdin from `/dev/null` (there is no
  stdin relay and no PTY), its own process group, the stagger slot, and the
  configured retry policy. Configured retries happen *inside* one session and
  show up as `attempt`; they never create a second session.
- **Single flight.** An exact replay of an idempotency key returns the original
  record and launches nothing. A new request for a worktree whose session is
  still running is `worktree_session_busy` with `reason=worktree_busy`. At most
  4 sessions run at once; the fifth is `worktree_session_busy` with
  `reason=capacity`, refused before any Git effect, so a refused
  `create_session_worktree` creates no worktree and no branch.
- **Provisioning matches `+`.** `create_session_worktree` creates a
  `ws-session-<random>` branch at base `HEAD` under the workspace base directory,
  validates the cwd after creation, runs `.wt/setup`, and validates again after
  setup and immediately before launch. When `.wt/setup` fails, the worktree is
  removed exactly as the TUI removes it — forced, teardown skipped — and the
  `ws-session-*` branch is **retained**; nothing is launched, and the record
  fails `worktree_session_setup_failed` with `result.cleanup` `removed` or
  `retained` plus a reason. An invalid cwd at any validation is
  `target_ineligible` with `reason=cwd_invalid` and launches nothing. The
  result's `session_kind` is `existing` or `session`.
- **Stop.** `stop_worktree_session` cancels through the runner's own handle:
  SIGTERM to the session's process group, SIGKILL after 5 seconds, verified
  group cleanup, and no further attempt even with retries configured. The
  record settles only after the leader is reaped and the cleanup report is read,
  with `result.terminated` and `result.state_after`; a session that had already
  finished settles `no_op`. Descendants that left the process group (`setsid`,
  daemonization) are outside this guarantee. Owner shutdown stops every running
  session the same way under a 10-second deadline. Scheduler `stop`,
  `stop_and_dequeue`, and `force_stop` never touch sessions.
- **Deletion is refused while a session runs.** `delete_worktree` — and the
  TUI's own deletion keys — refuse a worktree whose session is running with
  `target_ineligible` ("a worktree session is running"). Merging is not gated.
- **Retention.** Sessions are in memory only: running sessions are never
  evicted, at most 32 finished ones are kept (oldest evicted first), nothing is
  written to disk, and a restart forgets everything. Sessions never influence
  scheduling, resume, acceptance, or archive.
- **The output tail is the command's own data.** `output.lines` holds the
  newest 64 KiB of stdout/stderr lines (`truncated_bytes` counts what was
  dropped), passed through the same sanitizer as log lines: ANSI and control
  sequences stripped, each line bounded at 8 KiB, **nothing redacted**. If the
  configured command prints its arguments, its environment, or its cwd, every
  bearer-authenticated reader of the session resource sees them. Choosing a
  `worktree_command` that does not print secrets is the operator's
  responsibility. The runner's own stderr notices (a `[Retry N/M]` line before
  each configured retry, from which `attempt` is counted, and process-group
  cleanup diagnostics) are interleaved in the same tail. A stopped session
  reports `signal: "SIGTERM"`, the signal the cancellation path sends first;
  its SIGKILL escalation is not reported separately. The tail is published
  nowhere else: never as an event, a log
  entry, or a snapshot field. Owner-generated members (`path`, `branch`,
  results, error detail, events) never contain the template, the expanded
  command, environment values, or an absolute path.
- **Events.** `worktree_session_started` and `worktree_session_finished` carry
  `{session_id, worktree_id, state, exit_code}`. They take an event sequence but
  never advance `state_revision`. `GET /execution-status` reports
  `process.worktree_sessions_running`.
- **Older owners.** An owner built before this surface omits
  `capabilities.worktree_sessions` (decode it as unavailable) and answers each
  of the three commands with `validation_failed` before any effect. `cflx client`
  and `cflx client mcp` have no session verb.

The full schema is generated at runtime rather than tracked in the repository.
Export it with `cflx openapi > openapi.yaml`, or fetch it from a running
instance at `GET /api/v2/openapi.yaml`; both return the same document.

## Workflow Examples

### Example 1: Automated Full Run (Headless)

```bash
cflx run
```

### Example 2: Interactive TUI Workflow

```bash
cflx
```

### Example 3: Step-by-Step Processing

```bash
# Process first change
cflx run --change change-1

# Verify changes
openspec list

# Process second change
cflx run --change change-2
```

### Example 4: Resume After Interruption

```bash
# Run orchestrator (interrupted mid-run)
cflx run

# If interrupted, just run again - workspaces are automatically resumed
cflx run
```

### Example 5: Workspace Execution with Web Monitoring

```bash
# Run with the default workspace mode and web dashboard
cflx run --web

# Or use TUI with web monitoring
cflx --web
```

## Integration with CI/CD

### GitHub Actions

```yaml
name: Conflux Orchestrator

on:
  schedule:
    - cron: '0 */4 * * *'  # Every 4 hours
  workflow_dispatch:

jobs:
  orchestrate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Conflux
        run: cargo install --path .

      - name: Run orchestrator
        run: cflx run
        env:
          RUST_LOG: info
```

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM ubuntu:22.04
RUN apt-get update && apt-get install -y openssl ca-certificates
COPY --from=builder /app/target/release/cflx /usr/local/bin/
ENTRYPOINT ["cflx"]
```

Run:

```bash
docker build -t cflx .
docker run -v $(pwd):/workspace cflx run
```

## Troubleshooting Examples

### Debug Mode

```bash
RUST_LOG=debug cflx run 2>&1 | tee debug.log
```

### Verbose Output

```bash
RUST_LOG=trace cflx run --change test-change
```

## Best Practices

### 1. Use the TUI for Interactive Work

```bash
cflx
```

### 2. Use Headless Mode for Automation

```bash
cflx run
```

### 3. Incremental Processing

For safety, process one change at a time:

```bash
for change in $(openspec list --json | jq -r '.[].id'); do
  cflx run --change "$change"
  if [ $? -ne 0 ]; then
    echo "Failed on $change"
    break
  fi
done
```

### 4. Preview Before Running

```bash
cflx run --all --dry-run
cflx run --all
```

### 5. Monitor with Web UI

```bash
cflx --web
cflx run --web
```

### 6. Check Spec Conflicts Early

```bash
cflx check-conflicts
```

## Tips

- Default mode is the TUI: `cflx`
- Use `cflx run` for CI/CD and automated pipelines
- Every run executes changes in managed git worktrees; use `--max-concurrent` to bound concurrency
- Add `--web` for the HTTP dashboard
- Use `RUST_LOG=debug` for detailed logs

## Common Patterns

### Pattern 1: Nightly Automation

```bash
#!/bin/bash
# nightly-orchestrator.sh

cd /path/to/project
cflx run

STATUS=$?
if [ $STATUS -eq 0 ]; then
  echo "Orchestration completed successfully"
else
  echo "Orchestration failed with status $STATUS"
fi
```

### Pattern 2: Selective Processing

```bash
# Process only specific changes
cflx run --change urgent-fix,critical-update
```

### Pattern 3: Concurrent Execution with Monitoring

```bash
# Run with web monitoring reachable from other hosts
export CFLX_WEB_TOKEN="$(openssl rand -hex 32)"
cflx run --all --web --web-bind 0.0.0.0 --web-auth-token-env CFLX_WEB_TOKEN
```
