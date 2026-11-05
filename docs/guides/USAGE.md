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

#### Merging a worktree remotely

Two separately advertised commands merge a worktree's branch into base. Both
take only `target.worktree_id` and an empty `params` object, and both run the
same base merge (`git merge --no-ff`), root guard, and `on_merged` hook the TUI
uses. They differ only in what a conflict does:

- `merge_worktree` **preserves** a conflict. It fails `merge_conflict` with the
  conflicting files, leaves `MERGE_HEAD` and the conflicted index in place for
  local or TUI resolution (`recovery: local_or_tui_required`), and every later
  incompatible mutation is `root_busy` until that happens. Its behavior,
  parameters, and the capability `merge_conflict_preserves_state: true` are
  unchanged.
- `merge_worktree_abort_on_conflict` **aborts** a conflict, the way the TUI's
  `M` does. A conflict the fresh targeted inspection already predicts is
  refused as `merge_conflict_predicted` before any Git mutation, with the
  files in the result. A conflict that only appears when the merge runs gets
  exactly one `git merge --abort`, whose exit status is recorded, and then
  three repository reads decide whether the base was restored: `HEAD` equals
  the pre-merge commit, no `MERGE_HEAD` exists, and the root working directory
  is clean under the read-only status policy.

Restoration is reported from those reads, never from the abort attempt:

| `abort.restored` | error code | meaning |
|---|---|---|
| `confirmed` | `merge_conflict_aborted` | the merge failed; all three reads passed, so the root is at `premerge_head`, clean, with no `MERGE_HEAD`; another merge is admissible |
| `failed` | `merge_abort_failed` | every read answered and at least one failed — for example the abort exited non-zero and `MERGE_HEAD` is still there |
| `unknown` | `merge_abort_failed` | at least one read could not be answered |

The settled record's `result` has `kind: merge_abort` on success and on each
of those three refusals: `outcome` (`merged`, `predicted_conflict`,
`conflict_aborted`, `abort_failed`), `premerge_head`, `conflict_files`,
`abort` (`exit_ok`, `head_after`, `merge_head_present`, `clean_after`,
`restored`, `detail`), and `recovery`, which is `local_or_tui_required` only
for `merge_abort_failed`. A `merge_abort_failed` record never claims the state
was preserved on purpose; it reports what was observed. While `MERGE_HEAD`
remains, incompatible mutations are `root_busy` until local or TUI recovery.
If `MERGE_HEAD` is gone but the root is dirty, the next merge is refused by the
ordinary clean-working-directory check (`internal_error`, "Working directory
is not clean"), not reported as `root_busy`. The owner never retries the abort
and never runs `git reset` or any other compensation, and a server relaying
these commands performs none either. `on_merged` runs only after a completed
merge, exactly once.

Discover the command through `capabilities.worktrees.merge_abort`
(`available`, `command`, `predicted_conflict_refused`, `restore_verification`,
`reasons`) and the `merge_abort` entry in `worktrees.operations`. An owner
built before this command omits the block, which a client decodes as
`available: false`; such an owner refuses the command type with
`validation_failed` before any effect, and serves `merge_worktree` exactly as
before.

#### Editing a text file remotely

One bounded text surface lets a single operator read and save a file that
belongs to a change or to a managed worktree, the way the TUI `e` key opens
`$EDITOR` locally:

```bash
curl -H "$AUTH" "$BASE/changes/my-change/text?path=tasks.md"
curl -H "$AUTH" "$BASE/worktrees/$WORKTREE_ID/text?path=src/lib.rs"

curl -H "$AUTH" -H 'Content-Type: application/json' "$BASE/commands" -d '{
  "type": "write_worktree_text",
  "target": {"worktree_id": "'"$WORKTREE_ID"'"},
  "params": {"path": "src/lib.rs",
             "expected": {"kind": "content", "content_id": "sha256:…"},
             "content": "…"},
  "expected_revision": 41, "idempotency_key": "save-1"
}'
```

- **Roots are server-derived.** `write_change_text` / the change read address
  `openspec/changes/<change_id>` of the project for a tracked, non-archived
  change; `write_worktree_text` / the worktree read address the directory an
  opaque `worktree_id` is bound to. No root, absolute path, or ref is ever
  accepted from the client.
- **Limits.** A regular UTF-8 file of at most 1 MiB (1,048,576 bytes) without
  NUL, addressed by a `/`-separated relative path of at most 1024 bytes. An
  empty path, a leading `/`, `\`, an empty, `.` or `..` component, or a
  trailing `/` is `validation_failed` (422) before anything is touched. A
  missing intermediate directory is `not_found`; none is created.
- **Excluded paths** are `text_not_editable` (409) with a stable `reason`:
  `.git` as a file or a directory at any depth (`git_metadata`); anything under
  top-level `.wt/` and the top-level `.conflux-inflight` (`conflux_owned_path`
  — `sub/.conflux-inflight` is ordinary text); any symlink on the way, or a
  non-directory used as one (`unsafe_path`); any directory below the root that
  carries its own `.git` entry of any type — a nested worktree, a submodule, an
  embedded repository (`nested_repository`); a non-regular file
  (`not_regular_file`); a file with any execute bit (`executable`); more than
  1 MiB (`too_large`); invalid UTF-8 or NUL (`not_text`); a root that is
  missing or a symlink (`root_missing`). From the main worktree's ID, a file in
  another change's worktree placed under the project root is
  `nested_repository`: edit it through that worktree's own ID, where its own
  eligibility applies.
- **Content identity.** `content_id` is `sha256:` plus 64 lowercase hex digits
  over the exact bytes. Every write names what it read —
  `{"kind":"content","content_id":…}`, or `{"kind":"absent"}` to create a file
  that must not exist yet. There is no unconditional write. A mismatch is
  `text_stale`: nothing is written, and the record's
  `result.current_content_id` carries what the file holds now (`null` when it
  is absent), so a client keeps its buffer and re-reads. Identical content
  settles `no_op` with `outcome: unchanged` and opens nothing for writing.
- **The write itself** goes through a same-directory temporary file
  `.<leaf>.cflx-tmp-<16 hex>`, `fsync`, and `rename`, keeping the existing
  permission bits (a new file is `0644` subject to the umask). Every path
  component is re-walked with `O_NOFOLLOW` on the write, so a symlink
  substituted after a read is still refused. Any failure the process survives
  removes the temporary file and reports `effects.file_written: false`. A
  process killed between creating it and the rename can leave that
  `.cflx-tmp-` file behind: it is reported by Git and by every Conflux
  cleanliness check as ordinary untracked dirt, it is never ignored or swept,
  and you delete it by hand.
- **Saving never commits, stages, or pushes.** Git dirty state follows from the
  file write alone. After a successful worktree write the ordinary worktree
  observation is refreshed; when that changes what the owner observes, the
  state revision advances like any other refresh. **A save can therefore
  require a reload**: re-read `state_revision` before the next fenced command
  instead of reusing the pre-save value.

Writer exclusion is published, not hoped for. Every read reports `editable`,
`edit_blocked_reason`, and a bounded, diagnostic-only `edit_blocked_detail`
from the same classifier the command uses, and a write refused by it is
`target_ineligible` with the same reason on its `result`. Reasons, in the order
they are checked:

| reason | refused while |
|---|---|
| `change_active` | the change — for a worktree, the change that owns it — is in an active status (`preparing`, `applying`, `accepting`, `rejecting`, `archiving`, `resolving`) |
| `change_admitted` | it is admitted: queue intent `queued`, or display status `queued` or `blocked` — the scheduler may dispatch it at any moment, so a save would race the agent's first write |
| `managed_process_live` | the owner's managed writer coverage for the root is not `Clear` |
| `root_busy` | the repository root guard is held, a base merge is in progress, or a resolve is running |

Through the main worktree's ID, a path inside `openspec/changes/<id>/` of a
tracked change is judged by that change's own status, exactly as the change
scope is, so the project root is not a way around `change_active` or
`change_admitted`.

`managed_process_live` has two meanings, and `edit_blocked_detail` tells them
apart: *a managed writer is bound here* (a run-scoped command, an unscoped
shell, or a configured `worktree_command` session runs in this directory, in
one below it, or in one above it such as the project root), or *the owner
cannot prove that none is* (an identity it never proved quiescent, a writer
whose working directory could not be canonicalized, or no coverage source at
all). A previous run's processes that were never proven quiescent keep their
root blocked until existing lifecycle paths prove them quiescent; starting a
new run does not forget them. An owner restart is neither quiescence evidence
nor an authorized workaround — the coverage of a restarted owner says nothing
about processes it did not launch. The editor never kills, signals, sweeps, or
resets anything to make a root editable, and `force_stop_change` keeps its
exact target and rights.

What the compare-and-swap does **not** promise: the compare and the rename are
not one atomic step against external writers. An editor or tool outside
Conflux that writes between them is overwritten, and such external,
uncooperative editors are not excluded — nothing is locked with `flock`,
because no external editor honours it. The eligibility snapshot is taken once,
at the start of a write; a scheduler dispatch or a new writer that appears
after it and before the rename falls into the same window.

Discover the surface through `capabilities.text_edit` (`available`,
`read_routes`, `commands`, `max_bytes`, `max_path_bytes`,
`content_id_algorithm`, `excluded`, `reasons`) and the `text` entry in
`worktrees.operations`. An owner built before it omits the block, which a
client decodes as `available: false`; such an owner refuses either command
type with `validation_failed` before any effect and serves every other route
and command unchanged. `cflx client` and `cflx client mcp` have no text verb.

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
