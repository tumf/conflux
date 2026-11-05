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
component, file or directory), `conflux_hook_path` (top-level `.wt/`),
`unsafe_path` (a symlink or non-directory on the walk or a symlink leaf),
`not_regular_file`, `executable` (any execute bit), `too_large` (over
1,048,576 bytes), `not_text` (invalid UTF-8 or NUL), `root_missing` — and
with `target_ineligible` and reason `change_active`, `managed_process_live`,
or `root_busy` when the owning change is in an active display status, holds a
live managed process group, or the repository root mutation guard or an
unresolved base merge or resolve is in progress. The read response MUST
publish `editable` and `edit_blocked_reason` from the same classifier.

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
or push.

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

- **GIVEN** change `alpha` is `applying` or holds a live managed process group, or the root guard is held
- **WHEN** a client reads and then writes a file in `alpha`'s worktree
- **THEN** the read reports `editable: false` with the matching reason
- **AND** the write fails `target_ineligible` with the same reason and writes nothing

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
