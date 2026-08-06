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
