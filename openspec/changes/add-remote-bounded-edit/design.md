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
equals the bound path, if any), whether the owner holds a live managed process
group *bound to a path under the target root*, whether the root guard is
currently held, `base_merge_in_progress` and `is_resolving`. Reasons are
evaluated in table order and the first match is published:

| reason (`target_ineligible`) | change scope | worktree scope |
|---|---|---|
| `change_active` | `is_active_status(display_status)` | the owning change's status is active |
| `change_admitted` | `queue_intent == queued` or display status `queued` / `blocked` | the owning change is admitted the same way |
| `managed_process_live` | a live managed process group's working directory is the change directory or below it | a live managed process group's working directory is the worktree directory or below it |
| `root_busy` | root guard held, base merge in progress, or `is_resolving` | root guard held, base merge in progress, or `is_resolving` |

`change_admitted` follows `refresh_proposal`'s `target_admitted`: a `queued`
or `blocked` row is one the scheduler may dispatch at any moment without
passing the command application gate, so a save that landed between admission
and dispatch would race the agent's first write. The TUI editor has no such
gate only because its operator is local.

`managed_process_live` is path-bound, not change-bound. The termination port
gains `async fn owns_managed_process_under(&self, root: &Path) -> bool`,
answered from the same registry the kill walks, and the registry records the
`cwd` each managed command was spawned with on its `ScopeEntry`. A live group
whose working directory is the target root or a descendant of it blocks the
edit whatever its `change_id` is — including an entry with no change ID at
all, which is how a future configured session or any other non-change-keyed
managed process is covered without a dependency on it. A live group spawned
with no `cwd` runs in the owner's own working directory, the project root,
and therefore blocks the main worktree row and every change directory. The
existing change-keyed `owns_managed_process` and `actions.force_stop_change`
are unchanged.

`WorktreeService` gains `pub fn root_guard_is_held(&self) -> bool`
(a `try_lock` probe released immediately). The read response publishes
`editable` and `edit_blocked_reason` from the same classifier; a `worktree`
row with no owning change — the main worktree row included — is editable only
while no live managed process group is bound to a path under it and the root
is not busy by the same inputs the change scope uses.

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
  "editable": true, "edit_blocked_reason": null,
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

1. Resolve the scope root; classify eligibility; refuse `target_ineligible`
   with reason before touching the filesystem.
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
 "reason": null | "<text_not_editable or stale reason>",
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
change, a live managed process group bound to a path under the root, a held
root guard, a base merge or resolve in progress) rather than by the compare; (c)
external, uncooperative editors are *not* excluded, and the usage guide says
so. Nothing is locked with `flock`, because no external editor honours it.

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
snapshot and process-local facts on every request. The only file Conflux
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
  holding `ApplyStarted` for the owning change, and separately with a managed
  termination port reporting a live process group bound to the worktree path
  *with no change ID*, and separately with the service root guard held via
  `acquire_root_for_test`, and separately with `is_resolving` set on the main
  worktree row, both the read's `editable: false` reason and the command's
  `target_ineligible` reason agree; a port reporting a live group bound to a
  sibling path leaves the target editable.
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
`cargo test --lib --features web-monitoring remote_worktree`, and
`cargo test --lib --features web-monitoring force_stop_change` must pass
unchanged; the worktree capability `operations` assertion in
`remote_worktree_capabilities_report_the_operations_and_recovery_boundary`
is updated for the added `"text"` operation, and test doubles implementing
`ManagedProcessTermination` gain the added path-bound method; those are the
only expected edits to existing tests.

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
