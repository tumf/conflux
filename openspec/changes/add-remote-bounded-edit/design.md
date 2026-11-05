# Design: bounded remote text edit

## Source evidence at develop `62d256107`

- The TUI opens `$EDITOR` on `openspec/changes/<id>/proposal.md` (Changes
  view) or on the selected worktree directory (Worktrees view) through
  `handle_editor_launch` (`src/tui/key_handlers.rs:216-253`). It refuses while
  the worktree is being deleted and suspends the terminal; it has no path,
  size, or content policy because the editor is local.
- `/api/v2` addresses a worktree only by the process-local opaque ID bound to
  `(path, gitdir identity)` in `WorktreeRegistry`
  (`src/web/remote_control_api/worktrees.rs:40-170`); display paths are
  repository-relative and the canonical root is never serialized.
- Change-to-worktree relation and the project root live in
  `OperatorFactsStore` (`src/web/operator_facts.rs`), already exposed to the
  refresh preview through `RefreshPathSource` on `WebState`
  (`src/web/remote_control_api/proposal_refresh.rs:30-40`).
- Per-change `managed_process_live` is read from the managed-termination port
  at snapshot time (`src/web/state.rs:739-746`) and feeds
  `actions.force_stop_change`; the same fact is what decides whether an agent
  may be writing into a worktree.
- The repository mutation guard is `WorktreeService::root_guard`, taken with
  `try_lock` (`src/worktree_ops/service.rs:1313-1320`); `base_merge_in_progress`
  is an observed `SafetyFact` on every worktree row.
- `O_NOFOLLOW` opens are already used by the proposal refresh write
  (`src/orchestration/proposal_refresh.rs:1140`) with the declared `libc`
  dependency; `sha2` is a declared dependency.
- `.wt/setup` is read from the *project root* and made executable before it
  runs (`src/vcs/git/commands/worktree.rs:1162-1200`); `.wt/teardown` is read
  from the *worktree* and runs only when it is a regular executable file
  (`src/vcs/git/commands/worktree.rs:1003-1045`).
- A linked worktree's `.git` is a regular file holding a `gitdir:` pointer;
  the main worktree's `.git` is a directory.
- The main worktree is itself a v2 worktree row (`is_main: true`,
  `src/web/remote_control_api/worktrees.rs:284-328`), and managed worktrees
  can live *below* the project root (`repo_root.join(".worktrees")` in
  `src/orchestration/rejection.rs:687`; the default placement is
  configuration-dependent). A path walked from the main root can therefore
  reach another change's worktree, and a submodule or embedded repository can
  sit below any root.
- `.conflux-inflight` at the project root is read as a dependency-gate input
  (`src/openspec_cmd/dependency_status.rs:162-166`); it is a Conflux-owned
  path like `.wt/`.
- `ACTIVE_STATUSES` (`src/orchestration/operator_command.rs:64-71`) does not
  contain `queued` or `blocked`; admitted work is dispatched by the scheduler
  without passing the command application gate. `refresh_proposal` refuses
  admitted rows as `target_admitted`
  (`src/web/remote_control_api/proposal_refresh.rs:61`).
- The managed-process registry (`ScopeEntry`, `src/ai_command_runner.rs:123`)
  records `change_id: Option<String>` and the owned PGIDs, but not the
  working directory it was spawned with (`cwd`, `src/ai_command_runner.rs:897`
  and `:1860`), and `ManagedProcessTermination`
  (`src/orchestration/operator_command.rs:1704`) answers ownership by change
  ID only.
- The registry is not complete coverage of owner-managed writers, and the
  current-run view of it is not complete coverage of the registry:
  - `LiveRunManagedProcesses` (`src/orchestration/operator_command.rs:3333-3362`)
    re-reads *only* the supervisor's current scope slot and answers "owns
    nothing" when that slot is `None`. The supervisor overwrites the slot on
    every launch (`src/tui/run_supervisor.rs:404`) and empties it on shutdown
    (`take_run`, `:252`), so a previous run's `UnconfirmedRetained` entries
    (`ExecutionPhase`, `src/ai_command_runner.rs:100-112`) and the identities
    a timed-out `wait_quiescent` could not prove are no longer visible to the
    query even though the processes may still be alive.
  - `execute_streaming_with_retry` registers into the scope only when the
    runner carries one; with `run_command_scope == None` the spawn is
    unregistered (`src/ai_command_runner.rs:904-916`). The configured
    `worktree_command` session (`src/tui/terminal.rs:41-51`) calls exactly that
    path on the TUI's own runner with `change_id: None`, so an operator
    session running inside a worktree is invisible to the registry.
  - The shell path `execute_streaming` (`src/ai_command_runner.rs:1857`)
    spawns `sh -c` through the command queue and registers nothing at all.
  - A headless `cflx run` never installs a termination port on `WebState`
    (`src/web/state.rs:452-460`), and the snapshot maps that absence to
    `managed_process_live: false` (`src/web/state.rs:739-746`), which is
    correct for `force_stop_change` availability but is not evidence that no
    writer exists.
  - The external lifecycle adapter (`src/lifecycle_integration.rs:452`) is
    spawned without a working directory and is never handed a worktree path;
    its contract is a stdin event stream with stdout and stderr discarded, so
    it is not an owner-managed writer and is excluded from coverage by
    contract rather than by observation.
  - `.wt/setup` runs with `current_dir(worktree_path)` during worktree
    creation (`src/vcs/git/commands/worktree.rs:1216`) and `.wt/teardown`
    during deletion (`:1057`); both execute under the repository root guard,
    which `root_busy` already covers.

## Contract

### Scopes and roots

| scope | target | root | existence precondition |
|---|---|---|---|
| `change` | `change_id` (tracked, non-archived) | `<project_root>/openspec/changes/<change_id>` | directory exists, is not a symlink |
| `worktree` | opaque `worktree_id` | registered worktree path bound to the ID | registry resolves the ID; the directory is the bound path |

The root is canonicalized once and opened with
`O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`; a root that is a symlink or missing is
`text_not_editable` with reason `root_missing`. A retired or unknown worktree
ID is `worktree_not_found`; an untracked or archived change is `not_found`.

### Path rules

`path` is a UTF-8 string of at most 1024 bytes. Lexical validation (a
`validation_failed` 422 before any filesystem access) refuses: an empty path,
a leading `/`, any `\`, any empty, `.` or `..` component, a trailing `/`, and
any NUL. The remaining components are walked from the root descriptor with
`openat(dirfd, component, O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)` for every
directory component and `openat(dirfd, leaf, O_RDONLY | O_NOFOLLOW | O_CLOEXEC)`
for the leaf. Reasons published as `text_not_editable` (HTTP 409), evaluated in
this order and re-evaluated on every write:

| reason | condition |
|---|---|
| `git_metadata` | any component equals `.git` (ASCII case-insensitive) |
| `conflux_owned_path` | first component equals `.wt`, or the path is exactly the top-level `.conflux-inflight` |
| `unsafe_path` | an `openat` fails with `ELOOP`/`ENOTDIR` (a symlink or non-directory on the walk) or the leaf open fails with `ELOOP` |
| `nested_repository` | a *traversed* directory component (never the root itself) contains an entry named `.git` of any type — file, directory, or symlink — probed with `fstatat(dirfd, ".git", AT_SYMLINK_NOFOLLOW)` immediately after that component is opened |
| `not_regular_file` | the leaf `fstat` is not `S_IFREG` |
| `executable` | the leaf mode has any of `0o111` |
| `too_large` | the leaf size exceeds 1,048,576 bytes |
| `not_text` | the bytes are not valid UTF-8 or contain NUL |
| `root_missing` | the scope root cannot be opened as a non-symlink directory |

A leaf whose open fails with `ENOENT` is *absent*: a read answers
`exists: false`, `content: null`, `content_id: null`; a write with
`expected.kind = absent` may create it. A missing intermediate directory is
`not_found` (no directory creation is offered). A component that exists but is
not a directory is `unsafe_path`.

`nested_repository` is what keeps the main worktree's ID from reaching another
change's worktree, a submodule, or an embedded repository through an ordinary
relative path: every root's *own* `.git` (the main worktree's directory, a
linked worktree's file) is expected and is not examined, but the moment the
walk opens a descendant directory that carries its own `.git` entry the walk
stops, on read and on write alike, before the leaf is opened. Addressing a
nested worktree's file is refused with this reason even when the nested
worktree's owning change would itself be editable; the only way to edit it is
through its own opaque worktree ID, where its own eligibility applies.

### Content identity

`content_id` is the string `sha256:` followed by 64 lowercase hex digits of
SHA-256 over the raw file bytes. Reads compute it from the bytes they return.
Writes compute it from the bytes read through the same descriptor immediately
before the replace step. Test vector: the bytes `hello\n` identify as
`sha256:5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03`.

### Eligibility (writer exclusion)

`classify_text_eligibility` is one pure function used by both the read
resource and the command. Inputs: scope, the change's display status and
`queue_intent` (for a worktree, those of the change whose `worktree` relation
equals the bound path, if any), the owner's *managed writer coverage* for the
target root (the tri-state defined below, never a boolean), whether the root
guard is currently held, `base_merge_in_progress` and `is_resolving`. Reasons
are evaluated in table order and the first match is published:

| reason (`target_ineligible`) | change scope | worktree scope |
|---|---|---|
| `change_active` | `is_active_status(display_status)` | the owning change's status is active |
| `change_admitted` | `queue_intent == queued` or display status `queued` / `blocked` | the owning change is admitted the same way |
| `managed_process_live` | managed writer coverage for the change directory is `Busy` or `Unknown` | managed writer coverage for the worktree directory is `Busy` or `Unknown` |
| `root_busy` | root guard held, base merge in progress, or `is_resolving` | root guard held, base merge in progress, or `is_resolving` |

`change_admitted` follows `refresh_proposal`'s `target_admitted`: a `queued`
or `blocked` row is one the scheduler may dispatch at any moment without
passing the command application gate, so a save that landed between admission
and dispatch would race the agent's first write. The TUI editor has no such
gate only because its operator is local.

`managed_process_live` is path-bound, not change-bound, and it is answered
by coverage, not by a boolean. The existing change-keyed
`owns_managed_process`, `kill_managed_process`, and `actions.force_stop_change`
are unchanged: the kill still addresses the current run's scope by change ID,
and nothing in this design widens what may be signalled.

#### Managed writer coverage

```rust
pub enum WriterCoverage {
    /// Every owner-managed writer is accounted for and none is bound to the root.
    Clear,
    /// At least one accounted writer is bound to the root or below it.
    Busy,
    /// Coverage is incomplete: some writer's binding cannot be decided.
    Unknown { detail: String },
}
```

The termination port gains
`async fn writer_coverage_under(&self, root: &Path) -> WriterCoverage`.
`Busy` and `Unknown` both publish `managed_process_live`; only `Clear` lets
the classifier fall through to `root_busy`. No new public reason is
introduced. The read response and the refused record carry the coverage's
bounded `detail` text (at most 512 bytes, diagnostic only, never parsed by
clients) so an operator can tell "a process is running under this root" from
"the owner cannot prove nothing is", without a second read.

Coverage is computed by one process-local `ManagedWriterLedger`
(`src/ai_command_runner.rs`, beside `RunCommandScope`), created once per
owner process next to the run supervisor, handed to every `AiCommandRunner`
the process builds (`set_writer_ledger`, called where `set_run_command_scope`
is today), and read by `LiveRunManagedProcesses`. It is the union of these
sources, and every source is required — the ledger does not know how to say
"probably":

1. **Every run scope the supervisor ever installed.** `ScopeEntry` gains
   `cwd: WriterCwd` (`Known(PathBuf)` canonical, `OwnerCwd`, or
   `Unknown(reason)`), recorded by `register` *before* the spawn from the
   `cwd` argument. A scope registers itself with the ledger when the
   supervisor installs it, and the ledger keeps its clone until the scope is
   *resolved*: every entry released with confirmed quiescence, so that
   `entries` is empty. Replacing the current scope on the next launch, taking
   it on shutdown, a `CleanupUnconfirmed` outcome, or a timed-out
   `wait_quiescent` never drops a scope from the ledger; only positive proof
   — the confirmed-quiescence release, or a later `force_cleanup_retained`
   sweep that proves the retained identities empty — does. An entry in
   `WaitingToSpawn`, `Running`, `Cleaning`, or `UnconfirmedRetained` counts;
   an entry whose `unproven_pids` is empty but whose runner task has not
   ended counts too, because the spawn is imminent.
2. **Unscoped spawns.** The two paths that bypass the scope — the shell path
   `execute_streaming` and the `run_command_scope == None` branch of
   `execute_streaming_with_retry`, which is also the configured
   `worktree_command` session's path — take a `WriterLease { cwd }` from the
   ledger immediately before `Command::spawn`. A running lease with a known
   cwd is `Busy` for its bound root, not globally `Unknown` merely because
   the child has not exited. Release requires both a reaped direct child
   (the streaming task's wait or `ManagedChild::wait`) and positive proof of
   quiescence of its managed process group/retained descendants using the
   existing identity semantics. Reaping a shell alone is not proof that its
   writers exited. If that proof is unavailable, or a handle is dropped or
   wait fails, retain the lease as `Unknown` with bounded detail; no timer
   expires it. A spawn that fails returns the lease on the error path before
   the error is reported.
3. **The owner's own working directory.** `OwnerCwd` entries are spawns with
   no `cwd`, which run in the process's working directory. The ledger
   canonicalizes `std::env::current_dir()` once at creation; an `OwnerCwd`
   entry is `Busy` for every root the owner cwd is under or equal to, and
   `Unknown` for every root when that canonicalization failed.

Decision for a root `R` (canonical, as the scope root already is):

- `Unknown` if the ledger has any entry whose `cwd` is `Unknown`, or any
  lease whose completion/group quiescence is unresolved (not an ordinary
  running lease with known cwd), or if no ledger is attached to the port at all (an owner
  wired without one answers `Unknown` for every root, never `Clear`);
- else `Busy` if any entry's canonical `cwd` satisfies `cwd.starts_with(R)`
  (component-wise `Path::starts_with`, so `/w/alpha2` is not under
  `/w/alpha`), or is an `OwnerCwd` entry with the owner cwd under `R`;
- else `Clear`.

`Clear` therefore means "every writer this owner can launch has been
accounted for and none is bound here", and it is provable by a headless
`cflx run` with nothing running or a TUI that has never started a run — the
ledger exists and is empty — but never by the *absence* of a scope, a port,
or a ledger. A missing current scope is not evidence; an empty ledger is.
`cflx run` installs the same ledger-backed port on its `WebState` so its
reads publish `editable: true` when that is true; the snapshot's change-keyed
`managed_process_live` keeps its current `None => false` mapping because it
feeds `force_stop_change` availability, not writer exclusion.

Canonicalization: the `cwd` is canonicalized with `std::fs::canonicalize` at
registration, before the spawn, while the spawn itself keeps the path it was
given; failure (a dangling symlink, a directory removed before the spawn, a
permission error) records `Unknown(reason)` — never "no cwd", never absent.
The root is canonicalized the same way, so a symlink alias of a worktree on
either side compares equal to the real path and can neither clear nor
mis-attribute a binding.

Snapshot timing: coverage is read once, in write step 1, under the application
gate, together with the active/admitted facts — the eligibility snapshot. A
scheduler dispatch or a lease taken *after* that snapshot and before step 4's
rename is not detected; this is the same window as the external
compare-and-rename limitation below, it is bounded by the duration of one
write transaction, and no atomicity is claimed for it. What the snapshot does
close is the stale-view class: a writer that was already live, retained, or
unresolved when the snapshot was taken is always seen.

Not done, on purpose: no durable registry (the ledger dies with the process,
as the constitution requires, and a restarted owner begins with an empty
ledger and whatever processes outlived it are outside its coverage by
definition); no sweep or reset of retained entries to make a root editable;
no SIGTERM or SIGKILL sent to anything to clear coverage; no change to what
`force_stop_change` may signal. An operator facing `Unknown` resolves it
only through positive quiescence evidence from existing lifecycle paths,
not through the editor. Owner restart is neither a safety proof nor an
authorized remedy: this extension does not stop or restart owners, and
process-local coverage after a restart does not certify surviving writers.

`WorktreeService` gains `pub fn root_guard_is_held(&self) -> bool`
(a `try_lock` probe released immediately). The read response publishes
`editable`, `edit_blocked_reason`, and `edit_blocked_detail` from the same
classifier; a `worktree` row with no owning change — the main worktree row
included — is editable only while managed writer coverage for it is `Clear`
and the root is not busy by the same inputs the change scope uses.

### Read resource

`GET /api/v2/changes/{change_id}/text?path=...` and
`GET /api/v2/worktrees/{worktree_id}/text?path=...` return
`TextReadResponse`:

```json
{
  "instance_id": "…", "state_revision": 41,
  "scope": {"kind": "worktree", "worktree_id": "…"},
  "path": "src/lib.rs", "exists": true,
  "content": "…", "content_id": "sha256:…", "size_bytes": 1234, "mode": "0644",
  "editable": true, "edit_blocked_reason": null, "edit_blocked_detail": null,
  "limits": {"max_bytes": 1048576, "max_path_bytes": 1024, "content_id_algorithm": "sha256"}
}
```

A refusal is the ordinary `ApiError` with `error_code` `text_not_editable`,
`validation_failed`, `not_found`, or `worktree_not_found`. Reads are served on
both listeners, require the bearer token when one is configured, create no
record, and never advance the revision. A read is bounded by the leaf size
check before the bytes are read, so a 1 GiB file costs one `fstat`.

### Write commands

```json
{"type": "write_worktree_text",
 "target": {"worktree_id": "…"},
 "params": {"path": "src/lib.rs",
            "expected": {"kind": "content", "content_id": "sha256:…"},
            "content": "…"},
 "expected_revision": 41, "idempotency_key": "…"}
```

`write_change_text` is identical with `target.change_id`. `params` is
`TextWriteParams` with `deny_unknown_fields`; `expected` is a `kind`-tagged
enum (`content` with a `content_id` that must match `^sha256:[0-9a-f]{64}$`,
or `absent`); `content` is a JSON string, so it is UTF-8 by construction, and
is refused in `validate()` when it exceeds 1 MiB in UTF-8 bytes or contains
NUL. Both commands inherit revision fencing, idempotency identity, the
application gate, and `command_execution` availability. Admission order:
typed validation, idempotency, gate, revision, reservation, then the write
transaction below, exactly as in `commands.rs`.

Write transaction (under the application gate; no root guard is taken because
the write is file-local and the guard is only *probed*):

1. Resolve the scope root; take the eligibility snapshot (active, admitted,
   writer coverage, root busy) and classify; refuse `target_ineligible` with
   reason and detail before touching the filesystem.
2. Walk the path with `O_NOFOLLOW`; apply the path rules; open the parent
   directory descriptor and the leaf (or observe `ENOENT`).
3. Compare: `absent` expected and leaf present → `text_stale`
   (`current_content_id` set); `content` expected and leaf absent →
   `text_stale` (`current_content_id: null`); `content` expected and the leaf
   hashes differently → `text_stale` with the current identity. If the new
   bytes equal the current bytes → settle `no_op`, `outcome: unchanged`,
   nothing opened for writing.
4. Create `.<leaf>.cflx-tmp-<16 hex>` in the parent descriptor with
   `O_CREAT | O_EXCL | O_WRONLY | O_NOFOLLOW | O_CLOEXEC` and mode equal to
   the leaf's current mode (for `absent`, `0o644` subject to the process
   umask). Write all bytes, `fsync`, `renameat` over the leaf name, `fsync` the
   parent directory. Any failure unlinks the temporary file via `unlinkat` on
   the same descriptor and refuses with `internal_error`, result
   `effects.file_written: false`.
5. Re-read the leaf through a fresh `O_NOFOLLOW` open, report
   `content_id_after`, `bytes_written`, and `mode`.
6. For `worktree` scope, trigger the ordinary worktree observation refresh so
   `dirty` and the `Refreshed` worktree event follow; publish one state event
   `text_written` with payload `{scope, path, content_id_after}`. The write
   itself changes no snapshot member, so the event moves the sequence and not
   the revision — but the observation refresh it triggers is an ordinary
   observation, and when it changes `dirty` (a clean worktree that is now
   dirty) the revision advances exactly as it would for any other refresh. A
   client MUST therefore re-read `state_revision` before its next fenced
   command rather than assume the pre-write value; the usage guide says that a
   save can require a reload.

Result kind `text_write`:

```json
{"kind": "text_write", "scope": {...}, "path": "…",
 "outcome": "written" | "unchanged" | null,
 "reason": null | "<text_not_editable, target_ineligible, or stale reason>",
 "detail": null | "<bounded diagnostic; coverage detail for managed_process_live>",
 "content_id_before": "sha256:…" | null,
 "content_id_after": "sha256:…" | null,
 "current_content_id": "sha256:…" | null,
 "bytes_written": 1234 | null, "mode": "0644" | null,
 "effects": {"file_written": false}}
```

A refused record (`text_stale`, `text_not_editable`, `target_ineligible`)
carries the same result object, as `refresh_proposal` refusals do, so a
client reads `current_content_id` from the record rather than re-observing.

### What the compare-and-swap does and does not promise

The compare in step 3 and the rename in step 4 are not one atomic operation
against *external* writers. A write by an editor or agent that lands between
them is overwritten. The published guarantee is therefore: (a) a write never
succeeds against content the client did not read, as of the compare; (b)
Conflux's own writers are excluded by eligibility (an active or admitted
change, managed writer coverage that is not `Clear` for the root, a held root
guard, a base merge or resolve in progress) as of the eligibility snapshot in
step 1, rather than by the compare, and a writer the owner admits between that
snapshot and the rename falls into the same window as (c); (c) external,
uncooperative editors are *not* excluded, and the usage guide says so. Nothing
is locked with `flock`, because no external editor honours it.

### Capabilities, OpenAPI, errors

`CapabilitiesResponse.text_edit: TextEditCapability` with `#[serde(default)]`
(absent on older owners decodes as unavailable):

```json
{"available": true,
 "read_routes": ["/api/v2/changes/{change_id}/text", "/api/v2/worktrees/{worktree_id}/text"],
 "commands": ["write_change_text", "write_worktree_text"],
 "max_bytes": 1048576, "max_path_bytes": 1024,
 "content_id_algorithm": "sha256",
 "excluded": [".git", ".wt/", ".conflux-inflight", "nested-repository", "executable"],
 "reasons": ["git_metadata", "conflux_owned_path", "unsafe_path", "nested_repository",
             "not_regular_file", "executable", "too_large", "not_text", "root_missing",
             "change_active", "change_admitted", "managed_process_live", "root_busy"]}
```

`ErrorCode` gains `TextStale` and `TextNotEditable` (both HTTP 409, wire names
`text_stale`, `text_not_editable`) and `ALL_ERROR_CODES`, `SUPPORTED_COMMANDS`,
`SUPPORTED_V2_PATHS`, and the `CommandResult` enum are extended. The worktree
capability `operations` list gains `"text"`; `WorktreeCapabilities` otherwise
unchanged. `cflx client` decodes the new block through its existing
`#[serde(default)]` capability path and gains no verb.

### Constitution check

No durable state is introduced; eligibility is recomputed from the reducer
snapshot and process-local facts on every request. The managed writer ledger
is one of those process-local facts: it lives for exactly one owner process,
is never written to disk, and is not consulted for resume, acceptance, or
archive routing; killing the owner discards it, and the next owner's coverage
is whatever its own spawns and scopes register. The only file Conflux
creates inside a target is the temporary file that the operator's own write
consumes by rename; it is never retained by design, never ignored, and never
excluded from cleanliness checks (a failed write removes it before returning).
A process killed between step 4's create and its `renameat` can leave
`.<leaf>.cflx-tmp-<hex>` behind: that residue is reported by Git and by every
Conflux cleanliness check as ordinary untracked dirt, no sweeper removes it,
and the usage guide names the pattern so the operator can delete it by hand.
Refusing to hide it is the constitution's own rule, not a gap in this design.
The written file is the operator's deliverable, not Conflux runtime state.
`.conflux-inflight` and `.wt/` are excluded from remote edit for the same
reason in reverse: they are inputs Conflux executes or reads as control
state, and a remote save into them is not an operator deliverable.

## Rejected alternatives

- A single `write_text` command with a polymorphic target: breaks the
  `deny_unknown_fields` per-target structs that make smuggled fields a schema
  error.
- Git blob IDs as identity: a file that is untracked or unstaged has no blob
  without `hash-object`, and spawning Git per read is unnecessary when `sha2`
  is already a dependency.
- Unconditional writes or `expected: null` meaning "no check": every write must
  name what it read; `absent` is the only way to create.
- Extending `merge_worktree`-style params onto an existing command: there is
  no existing text command to extend.
- Allowing `.wt/` edits with a confirmation: these files are executed later by
  Conflux as the owner's UID; the TUI editor allows it only because the
  operator is local. Remote edit of executable or hook paths is out.
- Holding the root guard for the write: it would make every save contend with
  merges for no safety gain; the guard is probed for eligibility only.
- A boolean `owns_managed_process_under(root) -> bool` on the termination
  port: `false` would be read as "no writer" by the classifier while the only
  thing the current-run adapter can actually prove is "no writer *in the
  current scope*". A previous run's retained identities, an unscoped shell
  spawn, and a headless owner with no port all collapse into the same `false`.
  The tri-state keeps "nothing is bound" and "I cannot tell" apart, and maps
  both of the unsafe answers to a refusal.
- Dropping a retired scope when the next run starts, or after the quiescence
  budget expires: the identities are exactly the ones nobody proved dead, and
  the editor is the one consumer that must not forget them.
- Sweeping, killing, or resetting retained entries to make a root editable:
  the editor has no kill rights and gains none; `force_stop_change` keeps its
  exact target and its exact scope.
- A durable registry of spawned writers across owner restarts: forbidden by
  the constitution's first law, and a restarted owner cannot prove anything
  about processes it did not launch anyway.
- A string-prefix comparison of `cwd` against the root: `/w/alpha2` starts
  with the string `/w/alpha`; only component-wise `Path::starts_with` on two
  canonical paths answers containment.

## Verification design (future implementation, not executed at proposal time)

Test-name prefix `remote_text_edit` for every new case. Both declared gates
must report nonzero passed tests. Real-Git cases use the explicit heavy tier
with `#[cfg_attr(not(feature = "heavy-tests"), ignore)]` and the existing
isolated fixture pattern of `src/orchestration/proposal_refresh/tests.rs`
(`GIT_CONFIG_GLOBAL=/dev/null`, repository-local config, `TempDir`). Fast
cases stay under the repository's one-second target. Hang guards are
generous timeouts, never elapsed-time assertions.

### API gate: `remote-text-edit-api` (`src/web/remote_control_api/tests/mod.rs`)

New module `src/web/remote_control_api/tests/text_edit_tests.rs`, registered
in `tests/mod.rs`, wired like `proposal_refresh_tests::Wired` (real router,
reducer, `WebState`, worktree service over a disposable repository with one
linked worktree). Required cases:

- `remote_text_edit_read_reports_identity_mode_and_limits`: read a 0644 file;
  assert `content`, `content_id` equals the sha256 vector computed in the
  test, `size_bytes`, `mode`, `editable: true`, revision unchanged, no record.
- `remote_text_edit_write_replaces_bytes_atomically_and_dirties_git`: write
  with the read's identity; assert bytes on disk, mode preserved (`0600` →
  `0600`), `git status --porcelain` lists only the path, no commit, result
  fields, no `.cflx-tmp-` entry in the directory.
- `remote_text_edit_identical_content_is_a_no_op`: inode and mtime unchanged,
  record state `no_op`, `outcome: unchanged`.
- `remote_text_edit_stale_identity_writes_nothing`: change the file after the
  read; assert `text_stale`, `current_content_id` is the new identity, bytes
  unchanged, directory listing unchanged.
- `remote_text_edit_absent_expectation_creates_once`: `absent` creates the
  file with `0644 & !umask`; a second `absent` write is `text_stale`; a
  `content` write against a missing file is `text_stale` with
  `current_content_id: null`.
- `remote_text_edit_refuses_every_containment_violation`: table over lexical
  violations (422) and `text_not_editable` reasons: `.git` file in the linked
  worktree, `.git` directory in the main worktree, `.wt/teardown`,
  `.wt/setup` in change scope, top-level `.conflux-inflight` on the main
  worktree row (`conflux_owned_path`) while `sub/.conflux-inflight` stays
  editable, a symlink directory component, a symlink leaf, a leaf replaced by
  a symlink *after* a successful read, an executable file, a 1 MiB + 1 byte
  file, a file with a NUL byte, invalid UTF-8 bytes on read. Each case asserts
  no bytes changed anywhere under the root.
- `remote_text_edit_refuses_nested_worktree_through_main_root`: a linked
  worktree placed under the project root (`.worktrees/alpha`) and a directory
  holding a `.git` *symlink*; from the main worktree's ID, a read and a write
  of `.worktrees/alpha/src/lib.rs` and of `embedded/file.txt` are both
  `nested_repository`, while the same file read through `alpha`'s own ID is
  served; the digest of every file under the project root and under `alpha`
  is unchanged.
- `remote_text_edit_writer_exclusion_refuses_active_targets`: with the reducer
  holding `ApplyStarted` for the owning change, and separately with a fake
  termination port answering `Busy` for the worktree path (a lease *with no
  change ID*), and separately with the service root guard held via
  `acquire_root_for_test`, and separately with `is_resolving` set on the main
  worktree row, both the read's `editable: false` reason and the command's
  `target_ineligible` reason agree; a port answering `Clear` for the target
  while `Busy` for a sibling path leaves the target editable.
- `remote_text_edit_retained_scope_from_previous_run_blocks_edits`: a real
  `ManagedWriterLedger`-backed port over a `TuiRunSupervisor` whose run A scope
  holds an entry registered with `cwd = <alpha worktree>` and driven to
  `UnconfirmedRetained` through the existing `register_unproven_for_test`
  seam; run B is then launched through `install_run_for_test` with a fresh,
  empty scope. The read of a file in alpha reports `editable: false`,
  `managed_process_live`, and a detail naming the retained identity; the write
  is `target_ineligible` with no bytes changed; the sibling worktree stays
  editable; after the retained identity is released with confirmed quiescence
  (the test's own `release_confirmed`), the same read is `editable: true`.
  Nothing was killed and `force_stop_change` eligibility is unchanged
  throughout.
- `remote_text_edit_unbound_lifecycle_activity_refuses`: a port with no ledger
  attached (`LiveRunManagedProcesses` built without `with_writer_ledger`),
  and separately a ledger holding an unreaped lease, answer `Unknown` for the
  worktree, for the main root, and for a change directory; every read is
  `editable: false` with `managed_process_live` and a detail saying coverage is
  unknown, every write is `target_ineligible`, and the change-keyed
  `managed_process_live` published on the snapshot is unchanged (`false`)
  because coverage does not feed `force_stop_change`.
- `remote_text_edit_unscoped_spawn_paths_are_covered` (unit, in
  `src/ai_command_runner.rs`): a runner with `run_command_scope == None` and a
  ledger attached runs `execute_streaming("sleep 30", Some(wt))` and, in a
  second case, `execute_streaming_with_retry("sleep 30", Some(wt), None, None)`
  — the exact call the configured `worktree_command` session makes; while the
  child is alive the ledger answers `Busy` for `wt` and `Clear` for a sibling
  directory; the test terminates the child and, after the reap is observed
  through the handle, the ledger answers `Clear`. Liveness is proven by the
  reap plus confirmed group-quiescence evidence, not by elapsed time; a
  generous timeout is the hang guard only. A shell that exits while a managed
  descendant remains alive MUST retain coverage and refuse the write, until
  positive quiescence is observed; unavailable proof stays `Unknown`.
- `remote_text_edit_session_owned_writer_blocks_target`: a ledger entry with
  `change_id: None` and `operation: None` bound to the worktree path (the
  session shape) refuses the worktree row and the owning change's worktree
  alike, while the change directory under the project root stays editable; a
  lease with no `cwd` (owner cwd = project root) refuses the main worktree row
  and every change directory and leaves a worktree outside the project root
  editable.
- `remote_text_edit_cwd_canonicalization_decides_binding`: a lease registered
  through a symlink alias of alpha's worktree answers `Busy` for alpha and
  `Clear` for its sibling `alpha2` (the string-prefix trap) and for the
  project root; a lease whose `cwd` was removed before registration (a
  dangling path) records `Unknown` and refuses every root; a root reached
  through a symlink alias compares equal to the registered real path.
- `remote_text_edit_writer_exclusion_refuses_admitted_targets`: with the
  reducer holding the owning change as `queued` and separately as `blocked`
  (and separately `queue_intent = queued` on a `not queued` row), the read's
  `editable: false` and the command's `target_ineligible` both carry
  `change_admitted`, for the change directory and for the owned worktree;
  nothing is written.
- `remote_text_edit_change_scope_edits_only_the_change_directory`: write
  `openspec/changes/alpha/tasks.md` in the project root; a path outside the
  change directory via `..` is 422; a second change's directory and the
  worktree digest are byte-identical before and after.
- `remote_text_edit_cross_target_isolation`: SHA-256 digest of every file
  under the project root and under two worktrees before and after one
  worktree write differs only at the written path.
- `remote_text_edit_failure_leaves_no_temporary_file`: make the parent
  directory non-writable (skip when running as UID 0), assert
  `internal_error`, `effects.file_written: false`, and no temporary entry.
- `remote_text_edit_old_owner_capabilities_decode_as_unavailable`
  (in `dto_tests.rs`): a capabilities JSON without `text_edit` decodes with
  `available: false`; a `state` JSON without new members still decodes.
- `remote_text_edit_commands_are_closed_and_validated`: unknown param,
  missing `expected`, malformed `content_id`, oversize `content`, NUL in
  `content` are 422 with no record; replay of a settled write returns the
  original record without a second write (file mtime unchanged).

### Contract gate: `remote-text-edit-contract` (`tests/openapi_contract_tests.rs`)

- `remote_text_edit_contract_is_published_end_to_end`: the generated document
  contains both routes, both command variants with their `params` schema and
  the `expected` discriminator, the `text_write` result kind, both error
  codes, and `capabilities.text_edit`; the DTO values constructed field by
  field validate against the published schemas; the route surface and
  removed-path assertions still pass with `SUPPORTED_V2_PATHS` extended.
- `remote_text_edit_contract_omission_is_detected`: the negative control —
  removing the `text_write` result kind or one route from a copy of the
  document fails the completeness predicates.

### Non-declared regression runs the implementer must still report

`cargo test --features web-monitoring --test client_cli_tests`,
`cargo test --lib --features web-monitoring remote_worktree`,
`cargo test --lib --features web-monitoring force_stop_change`, and
`cargo test --lib run_command_scope` must pass unchanged; the worktree
capability `operations` assertion in
`remote_worktree_capabilities_report_the_operations_and_recovery_boundary`
is updated for the added `"text"` operation, and test doubles implementing
`ManagedProcessTermination` gain the added `writer_coverage_under` method
(the existing doubles answer `Clear` so their force-stop cases keep their
meaning); those are the only expected edits to existing tests.

## Independent review launch contract (for the implementation worker)

Independent implementation reviews are launched exactly as follows and never
through the built-in Agent/Task tools or a direct `claude` session:

```text
PATH=/home/tumf/.local/bin:/home/tumf/.cargo/bin:/home/tumf/bin:/usr/local/bin:/usr/bin:/bin
HOME=/home/tumf
/home/tumf/.cargo/bin/agent-exec run --until 100 --stdin-file /tmp/<unique-UUID>.md -- \
  /home/tumf/.local/bin/claude-auto-fable --output-format stream-json --verbose \
  --include-partial-messages --dangerously-skip-permissions --print
```

- No `--model` flag. The review session is separate from the implementation
  session and reads only the assigned diff plus compatibility; it is not a
  repository audit.
- The worker returns the review job ID, the raw log path, the exact argv, the
  actually responding model, and the final answer. A running job is neither
  killed nor duplicated.
- The parent owns acceptance; a wrapper or `wait` exit status `0` is not
  proof. No push, release, install, owner replacement, or global
  configuration edit is performed by the worker or the reviewer.
