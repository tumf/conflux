## ADDED Requirements

### Requirement: Bounded remote text edit API

V2 MUST expose `GET /api/v2/changes/{change_id}/text?path=<relative>` and
`GET /api/v2/worktrees/{worktree_id}/text?path=<relative>` as reads that
create no command record, advance no revision, and are served on both
listeners and by a headless `cflx run`. It MUST extend the closed command set
with `write_change_text` (target `change_id`) and `write_worktree_text`
(target opaque `worktree_id`), each taking exactly `params.path`,
`params.expected` (`{"kind":"content","content_id":"sha256:<64 hex>"}` or
`{"kind":"absent"}`), and `params.content`, with `deny_unknown_fields`,
ordinary revision fencing, idempotency identity, and the application gate.
There MUST be no unconditional write and no client-supplied root, absolute
path, branch, or Git ref.

The scope root MUST be server-derived: `openspec/changes/<change_id>` in the
project repository for a tracked, non-archived change, or the registered
directory bound to the opaque worktree ID. `path` MUST be at most 1024 UTF-8
bytes and MUST be refused as `validation_failed` when it is empty, absolute,
contains `\`, a NUL, or an empty, `.`, or `..` component. Every directory
component and the leaf MUST be opened with `O_NOFOLLOW` from the root
descriptor on every read and every write. The owner MUST refuse with
`text_not_editable` and a stable reason — `git_metadata` (any `.git`
component, file or directory), `conflux_owned_path` (top-level `.wt/` or the
top-level `.conflux-inflight`), `unsafe_path` (a symlink or non-directory on
the walk or a symlink leaf), `nested_repository` (a traversed directory below
the root that contains a `.git` entry of any type; the root's own `.git` is
not examined), `not_regular_file`, `executable` (any execute bit),
`too_large` (over 1,048,576 bytes), `not_text` (invalid UTF-8 or NUL),
`root_missing` — and with `target_ineligible` and reason `change_active`,
`change_admitted`, `managed_process_live`, or `root_busy` when the owning
change is in an active display status, is admitted (`queue_intent` queued or
display status `queued` / `blocked`), when the owner's managed writer
coverage for the target root is not `Clear`, or when the repository root
mutation guard is held or a base merge or resolve is in progress — the last
applying to the main worktree row and every change directory alike. The read
response MUST publish `editable`, `edit_blocked_reason`, and a bounded
diagnostic `edit_blocked_detail` from the same classifier. A nested worktree
MUST be editable only through its own opaque worktree ID, never through a
path from another root.

Managed writer coverage MUST be a tri-state (`Clear`, `Busy`, `Unknown`)
computed from one process-local ledger, never a boolean, and `Busy` and
`Unknown` MUST both refuse as `managed_process_live`. The ledger MUST account
for every owner-managed writer: every run command scope the owner has
installed, retained until each of its identities is proven quiescent and
never dropped because a later run replaced it, a shutdown took it, or a
quiescence budget expired; every spawn that bypasses the scope, including
the shell command path and the configured worktree session, held as a lease
from before the spawn until the child's exit is reaped; and spawns with no
working directory, bound to the owner's own canonical working directory.
Each writer's working directory MUST be canonicalized at registration and
compared to the canonical root by component-wise path containment, so a
symlink alias binds the real path and a sibling whose name shares a prefix
is not bound. A failed canonicalization, an unreaped lease, or an owner with
no ledger attached MUST be `Unknown`; the absence of a current scope, of a
termination port, or of a ledger MUST NOT be read as `Clear`. `Clear` MUST be
provable by an owner — a headless `cflx run` included — whose ledger exists
and is empty. The coverage MUST be read once, as part of the eligibility
snapshot at the start of a write, and the contract MUST state that a writer
admitted after that snapshot and before the rename is in the same window as
an external writer. Reaching `Clear` MUST NOT kill, sweep, or reset any
process or ledger entry, and `force_stop_change`, `owns_managed_process`,
and `kill_managed_process` MUST keep their current target and scope.

`content_id` MUST be `sha256:` followed by the lowercase hex SHA-256 of the
exact file bytes. A write MUST compare the current bytes to `expected`
immediately before replacing them, MUST refuse `text_stale` with the current
`content_id` (or `null` when absent) when they differ or when `absent` was
expected and the file exists, MUST settle `no_op` with `outcome: unchanged`
when the new bytes equal the current bytes, and otherwise MUST write through a
same-directory temporary file, `fsync`, and `rename`, preserving the existing
permission bits (`0644` subject to umask for a created file), removing the
temporary file on every failure path, and reporting a `text_write` result
with `outcome`, `content_id_before`, `content_id_after`, `current_content_id`,
`bytes_written`, `mode`, and `effects.file_written`. A refused record MUST
carry the same result. A successful worktree write MUST trigger the ordinary
worktree observation refresh so `dirty` follows, and MUST NOT stage, commit,
or push. The write itself MUST NOT advance the state revision, but the
observation refresh it triggers MAY, and the contract MUST tell clients to
re-read `state_revision` after a save. A temporary file left by a process
crash between create and rename MUST be reported as ordinary untracked dirt
and MUST NOT be ignored or excluded from any cleanliness check.

The contract MUST state that the compare and the rename are not atomic
against external writers: Conflux's own writers are excluded by eligibility,
and uncooperative external editors are not excluded. Capabilities MUST
advertise `text_edit` (`available`, `read_routes`, `commands`, `max_bytes`,
`max_path_bytes`, `content_id_algorithm`, `excluded`, `reasons`), and the
generated OpenAPI document MUST include both routes, both command variants,
the result kind, and both error codes. The block MUST decode as unavailable
when an older owner omits it, every existing route and command MUST remain
unchanged on such an owner, and a new command submitted to it MUST fail
typed validation before any effect. `cflx client` and `cflx client mcp` MUST
gain no verb.

#### Scenario: Read publishes identity and eligibility

- **GIVEN** a live opaque worktree whose `src/lib.rs` is a regular `0644` file
- **WHEN** an authenticated client reads `/api/v2/worktrees/{id}/text?path=src/lib.rs`
- **THEN** it receives `content`, `content_id` equal to `sha256:` of the bytes, `size_bytes`, `mode: "0644"`, `editable: true`, and the limits
- **AND** the state revision is unchanged and no command record exists

#### Scenario: Matching write replaces atomically and dirties Git

- **GIVEN** a prior read returned identity `C` for a `0600` file
- **WHEN** `write_worktree_text` is submitted with `expected.content_id = C` and new content
- **THEN** the file holds the new bytes with mode `0600`, the record settles `succeeded` with `outcome: written`
- **AND** `git status --porcelain` lists only that path and no commit is created
- **AND** no temporary file remains in the directory

#### Scenario: Stale write refuses and preserves client text

- **GIVEN** the file changed after the client read identity `C`
- **WHEN** a write with `expected.content_id = C` is submitted
- **THEN** the record fails with `text_stale` and `current_content_id` names the current bytes
- **AND** the file bytes and directory listing are unchanged
- **AND** no response asks the client to discard its buffer

#### Scenario: Containment refuses symlinks and Git metadata in both forms

- **GIVEN** a linked worktree whose `.git` is a file and a main worktree whose `.git` is a directory
- **WHEN** a client addresses `.git`, a path through a symlinked directory, a symlink leaf, `.wt/teardown`, an executable, a file over 1 MiB, or a file with NUL bytes
- **THEN** each is refused with `text_not_editable` and its stable reason
- **AND** a leaf replaced by a symlink after a successful read is refused at write time
- **AND** no bytes under either root change

#### Scenario: Active writers exclude remote edits

- **GIVEN** change `alpha` is `applying`, or the owner's writer coverage for `alpha`'s worktree is `Busy` (a registered writer with or without a change ID), or the root guard is held, or a resolve is running
- **WHEN** a client reads and then writes a file in `alpha`'s worktree
- **THEN** the read reports `editable: false` with the matching reason
- **AND** the write fails `target_ineligible` with the same reason and writes nothing
- **AND** a writer bound only to a sibling path leaves the target editable

#### Scenario: Retained writers from a previous run exclude remote edits

- **GIVEN** a previous run's scope holds an identity spawned under `alpha`'s worktree that was never proven quiescent, and a new run has since been started with a fresh scope
- **WHEN** a client reads and then writes a file in `alpha`'s worktree
- **THEN** the read reports `editable: false` with `managed_process_live` and a detail naming the retained identity
- **AND** the write fails `target_ineligible` with the same reason and writes nothing
- **AND** a sibling worktree stays editable, nothing is killed, and `actions.force_stop_change` is unchanged
- **AND** once the identity is released with confirmed quiescence the same read reports `editable: true`

#### Scenario: Unknown writer coverage fails closed

- **GIVEN** an owner with no writer ledger attached to its termination port, or a ledger holding a lease whose child was never reaped, or a writer whose working directory could not be canonicalized
- **WHEN** a client reads a file in any worktree, in the main worktree, or in a change directory
- **THEN** the read reports `editable: false` with `managed_process_live` and a detail saying coverage is unknown
- **AND** the write fails `target_ineligible` and writes nothing
- **AND** the change-keyed `managed_process_live` on the state snapshot is unchanged

#### Scenario: Unscoped and session writers are covered

- **GIVEN** a child spawned through the shell command path or through the configured worktree session with `alpha`'s worktree as its working directory and no change ID
- **WHEN** a client reads a file in `alpha`'s worktree while the child is alive, and again after its exit was reaped
- **THEN** the first read reports `editable: false` with `managed_process_live` and a sibling directory is `Clear`
- **AND** the second read reports `editable: true`

#### Scenario: Canonical containment decides a binding

- **GIVEN** a writer registered through a symlink alias of `alpha`'s worktree, and a sibling worktree whose name begins with `alpha`
- **WHEN** a client reads a file in each
- **THEN** `alpha` reports `editable: false` with `managed_process_live` and the sibling reports `editable: true`
- **AND** a writer whose working directory no longer exists at registration refuses every root

#### Scenario: Admitted targets exclude remote edits

- **GIVEN** change `alpha` is `queued` or `blocked`, or its `queue_intent` is queued
- **WHEN** a client reads and then writes a file in `alpha`'s change directory or in its worktree
- **THEN** the read reports `editable: false` with reason `change_admitted`
- **AND** the write fails `target_ineligible` with `change_admitted` and writes nothing

#### Scenario: Nested repository is not reachable through another root

- **GIVEN** change `alpha`'s worktree lies under the project root, and a directory under the main worktree carries its own `.git` entry
- **WHEN** a client addresses a file inside either through the main worktree's opaque ID
- **THEN** the read and the write are refused with `text_not_editable` and reason `nested_repository`
- **AND** the same file is served through `alpha`'s own worktree ID
- **AND** no bytes under the project root or under `alpha`'s worktree change

#### Scenario: Absent expectation creates exactly once

- **GIVEN** `notes.md` does not exist in the change directory
- **WHEN** `write_change_text` is submitted with `expected.kind = absent`
- **THEN** the file is created and the record reports `content_id_before: null`
- **AND** a second `absent` write is `text_stale`

#### Scenario: Older owner stays compatible

- **GIVEN** an owner built before this surface
- **WHEN** a client decodes its capabilities and submits `write_worktree_text`
- **THEN** `text_edit` decodes as unavailable
- **AND** the command is `validation_failed` with no record and no effect
- **AND** every existing read and command keeps working

#### Scenario: Contract completeness

- **GIVEN** a client captures `cflx openapi`
- **WHEN** it inspects the document
- **THEN** both text routes, both write commands, the `text_write` result kind, `text_stale`, `text_not_editable`, and `capabilities.text_edit` are present
- **AND** removing any of them from a copy fails the completeness check
