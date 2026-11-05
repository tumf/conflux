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

**Remote clients reach these escalations only through explicit consent.**
Ordinary remote deletion is unchanged: `/api/v2` `delete_worktree` and the Web
UI are fail-closed, teardown is mandatory, and a dirty or ahead worktree is
reported as undeletable with a reason. The same three escalations — skip
teardown (`S`), discard known uncommitted changes, and discard commits ahead
including the branch (`X`) — are reachable remotely only through the separate
`delete_worktree_confirmed` command, each as its own boolean, and only against a
`consent` object read from `GET /api/v2/worktrees/{worktree_id}/delete-preview`
and re-verified immediately before teardown and again before removal. See
[Deleting a worktree remotely](#deleting-a-worktree-remotely).

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
- Progress and timing come from three snapshot members, all process-local and
  zero, `false`, or empty again after the owner restarts:
  - `changes[].archive_complete` is `true` exactly when the reducer recorded
    archive completion for the change in this process. It is not
    `display_status`: an archived change truthfully advances to `resolving`,
    `resolve pending`, `merge wait`, `merged`, or `pushed`, while a
    fresh-process resolve retry shows `resolving` with `archive_complete:
    false`. It is not merge or push success either.
  - `changes[].timing` is *per run*: `processing_started` discards the previous
    run's boundaries and `elapsed_ms` is the finished run's duration.
    `changes[].net_active` is the *cumulative* counterpart, the same net
    execution time the TUI row shows: closed active intervals in
    `accumulated_ms`, plus the open one starting at `active_since` (`null`
    while closed). An interval opens on a transition into `preparing`,
    `applying`, `accepting`, `rejecting`, `archiving`, or `resolving`, closes on
    any other status or on process `Stopped`, and is never reset while the
    owner runs.
  - `run_clock` is the TUI `Elapsed` line, boundary for boundary: an accepted
    Start while the owner is not running opens it (`started_at` set,
    `ended_at` and `elapsed_ms` `null`); a persistent-idle park, persistent-idle
    re-admission, `ResolveReserved`, and a fatal process error leave it open;
    every `Stopped`, every `AllCompleted`, and a resolve settlement that leaves
    nothing processing freeze it, and a repeated terminal event re-freezes it at
    the later instant. The value stays until the next opening Start.

  No member is a live counter, so time passing never advances `state_revision`.
  Derive live values yourself: `accumulated_ms + (now - active_since)` while
  `active_since` is set, and `now - started_at` while `ended_at` is `null`. An
  owner built before these members omits them; decode the absence as
  "unpublished", never as `false` or zero.
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

#### Deleting a worktree remotely

`delete_worktree` (empty `params`) never changes: it always runs teardown,
refuses a dirty worktree as `worktree_dirty`, an unobservable one as
`worktree_dirty_unknown`, and an ahead or main worktree as `target_ineligible`,
and a `delete_worktree` carrying `skip_teardown` or any discard field is
`validation_failed`. Its capability `delete_requires_teardown: true` still
describes exactly that command.

Destructive deletion is a two-step, consent-bound flow:

1. **Preview.** `GET /api/v2/worktrees/{worktree_id}/delete-preview` is a read —
   no command record, no revision change, nothing written to the worktree. It
   takes a fresh targeted observation and returns `branch`, `head`, `is_main`,
   `dirty`, `has_commits_ahead`, `inspection`, `content_observation`,
   `has_ignored_content`, `ignored_entries`, `teardown_script_present` (true
   exactly when `.wt/teardown` is a regular file with an execute bit), the
   eligibility of each option (`ordinary`, `skip_teardown`,
   `discard_known_dirty` with `entries`, `discard_commits_ahead` with `commits`
   and `also_discards_dirty`) with a typed `blocked_reason`, and a `consent`
   object. `observed_at` is informational and is not part of `consent`.
   Cancelling is simply not submitting anything.
2. **Command.** `delete_worktree_confirmed` takes `target.worktree_id` and
   `params` with three independent booleans — `skip_teardown`,
   `discard_known_dirty`, `discard_commits_ahead` — plus the preview's
   `consent` echoed verbatim. Either discard permission without both content
   identities, `discard_commits_ahead` without `merge_base`, or a commit ID that
   is not 40 lowercase hex is `validation_failed` before anything is observed.
   All three booleans `false` is an ordinary delete with consent revalidation.
   When the preview's `discard_commits_ahead.also_discards_dirty` is `true`,
   that option's eligibility assumes `discard_known_dirty` is sent too; sending
   only `discard_commits_ahead` then refuses `worktree_dirty`.

**What consent binds.** Exactly eight fields: `branch`, `head`, `dirty`,
`has_commits_ahead`, `has_ignored_content`, `merge_base`,
`discardable_content_id`, and `ignored_content_id`, plus the Git identity the
opaque ID is bound to. The two identities are `sha256:` over the *bytes*
`git worktree remove --force` would delete beyond `HEAD`:

- `discardable_content_id` covers every staged, unstaged, unmerged, and
  untracked record of `git status --porcelain=v2 -z --untracked-files=normal
  --ignored=traditional`, and the on-disk bytes of each path (a symlink by its
  target string, never followed). A `README` that moved from `A` to `B` under
  the same `HEAD` and the same `dirty: true` is a different identity.
- `ignored_content_id` covers every ignored path. Forced removal deletes
  ignored paths too, so ignored content is disclosed (`has_ignored_content`,
  `ignored_entries`) and needs `discard_known_dirty` **even when the worktree is
  Git-clean**; `dirty` itself keeps its `--ignored=no` meaning. Ignored files are
  bound by their bytes, with type, size, mtime, and mode bound in addition —
  never metadata alone, so a same-size rewrite with the mtime restored changes
  the identity.
- Collapsed directories (`? dir/`, `! dir/` such as `target/`) are walked by the
  oracle itself, so files at any depth, empty subdirectories, special files,
  and symlinks beneath them are bound. Every path is resolved from an open
  worktree descriptor with `O_NOFOLLOW`; files are opened non-blocking, so a
  FIFO is recorded by type and never read, and no symlink is traversed.

**What refuses, and how.** Under the repository root guard, immediately before
teardown and again immediately before removal, every consented fact and both
identities are recomputed. Nothing is removed on any refusal, the worktree and
its opaque ID are retained, and the settled record's `result` has
`kind: delete_worktree_confirmed` with `refusal.reason`, `refusal.teardown_ran`,
and `refusal.observed` — the fresh facts — so a client can re-render its
confirmation:

| error code | `refusal.reason` |
|---|---|
| `delete_consent_stale` | `branch_changed`, `identity_changed`, `head_moved`, `base_moved`, `dirty_changed`, `ahead_changed`, `ignored_changed`, `content_changed` |
| `worktree_dirty_unknown` | `dirty_unknown`, `content_unobservable`, `content_unbounded` |
| `target_ineligible` | `ahead_unknown`, `nested_repository`, `main_worktree`, `commits_ahead` |
| `worktree_dirty` | `dirty`, `ignored_content` |
| `root_busy` | `root_busy` (a held root guard or a base merge in progress) |
| `internal_error` | `teardown_failed` |

Unknown is never safe: an unobservable dirty, ahead, or content fact refuses
whatever permissions were given. Two content outcomes fail closed the same way.
An actual nested repository — a `.git` entry at any depth, untracked or inside
an ignored directory, or a status-reported submodule — is `nested_repository`;
a plain ignored `target/` is walked, not refused. And one combined budget
covers tracked, untracked, and ignored content together, each path counted
once: more than 500,000 records (status records plus walked entries) or
256 MiB of hashed bytes is `content_unbounded`, with both identities `null`.
No metadata-only, partial, or ignored-excluding identity is ever substituted to
keep the command available. A worktree past the budget — typically a large
`target/` or other build output — is **not deletable remotely**: reduce that
output locally (clean it in place, or run the project's clean or `.wt/teardown`
yourself), or delete through the TUI, then preview again.

**Teardown.** Without `skip_teardown`, `.wt/teardown` runs (`teardown: ran`, or
`absent` when there is no executable script) and the facts are re-derived
afterwards. Teardown exists to clean build output, so the ignored identity may
afterwards be the consented one *or* empty; any other ignored set, and any
change to tracked or untracked content, is `delete_consent_stale`
(`content_changed`) with `teardown_ran: true`. Teardown effects are never
reversed. A failing teardown is `internal_error` (`teardown_failed`) with the
worktree retained; recover with a fresh preview and `skip_teardown: true`.

**Success.** `outcome: deleted`, `teardown` (`ran`, `skipped`, `absent`),
`dirty_discarded`, `commits_discarded`, `branch_deleted`, and
`branch_retained_reason`, and the opaque ID is retired. `discard_commits_ahead`
force-deletes the branch only at the consented commit, by atomic
compare-and-delete; a permission the observed facts do not need changes
nothing. Nothing is stashed, committed, backed up, or pushed first.

**Guarantee boundary.** Nothing is locked between the preview and the command.
The deletion proceeds only if every consented fact still matches under the root
guard immediately before teardown and immediately before removal. A write from
outside Conflux that lands after that final comparison and before
`git worktree remove --force` is not detected — the same window the TUI has.
Empty directories and lone special files that lie under no reported directory
record are listed by no status mode, hold no bytes, and are not bound. A
populated submodule that status does not report is outside the oracle; Git's
own `worktree remove` refuses such a worktree, which surfaces as
`internal_error` with the worktree retained.

Discover the surface through `capabilities.worktrees.confirmed_delete`
(`available`, `command`, `preview_route`, `permissions`, `consent_fields`,
`content_id_algorithm`, `binds_ignored_content`, `reasons`) and the
`delete_confirmed` entry in `worktrees.operations`. An owner built before this
surface omits the block, which a client decodes as `available: false`; it
refuses `delete_worktree_confirmed` with `validation_failed` before any effect,
serves `delete_worktree` and every other command unchanged, and still publishes
`delete_requires_teardown: true`.

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
