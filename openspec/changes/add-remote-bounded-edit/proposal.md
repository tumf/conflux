---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - openspec/specs/tui-editor/spec.md
  - docs/guides/USAGE.md
verifications:
  - id: remote-text-edit-api
    requirement: Bounded UTF-8 text read/write over cflx-issued change and worktree identity with content-identity CAS, path containment, and writer-exclusion eligibility
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/mod.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-text-edit-contract
    requirement: Generated OpenAPI, capabilities, and error-code vocabulary publish the text-edit surface completely and old-owner decoding stays tolerant
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: cargo test --features web-monitoring --test openapi_contract_tests remote_text_edit -- --nocapture
    rerun: cargo test --features web-monitoring --test openapi_contract_tests remote_text_edit -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add remote bounded text edit

**Change Type**: implementation

## Problem / Context

Conflux-server (CFSRV-50, OP-21 / OP-33) needs to let a single operator read
and save a text file that belongs to an OpenSpec change or to a managed
worktree from the Web, the way the TUI `e` key opens `$EDITOR` on
`openspec/changes/<id>/proposal.md` or on a worktree directory
(`src/tui/key_handlers.rs:216-253`, `openspec/specs/tui-editor/spec.md`).
At develop `62d256107` the `/api/v2` surface has no file read or write
resource: `/api/v2/changes/*` and `/api/v2/worktrees/*` are observations only,
and `GET /api/v2/proposals/{change_id}/refresh-preview` is a read of one
committed blob, not an editor. The accepted requirement note
(`Confluxの公開API拡張要求.md`, section 1 and its Conflux-side review)
conditions the feature on: cflx-issued identity only, a defined editable
scope and size ceiling, relative-path and symlink containment including
`.git` as a *file* in linked worktrees, content identity compare-and-swap,
atomic writes that preserve mode bits, an explicit policy for files Conflux
executes later (`.wt/setup`, `.wt/teardown`), and an explicit answer to the
agent/editor write race that blob comparison alone cannot close.

## Proposed Solution

Add one bounded text surface to `/api/v2`, owned entirely by cflx:

- Reads: `GET /api/v2/changes/{change_id}/text?path=<relative>` and
  `GET /api/v2/worktrees/{worktree_id}/text?path=<relative>`. Both are reads:
  no command record, no revision change, served on both listeners and by a
  headless `cflx run`.
- Writes: two closed commands, `write_change_text` (target `change_id`) and
  `write_worktree_text` (target opaque `worktree_id`), each with typed params
  `{path, expected, content}` where `expected` is either
  `{"kind":"content","content_id":"sha256:<hex>"}` from a prior read or
  `{"kind":"absent"}` to create a file that must not exist yet. There is no
  unconditional write.
- Scope roots are server-derived: `<project_root>/openspec/changes/<change_id>`
  for a tracked, non-archived change, and the registered worktree directory
  for a live opaque worktree ID. The client never names a root, an absolute
  path, or a Git ref.
- Content identity is `sha256:` over the exact file bytes, computed in process
  with the existing `sha2` dependency. A write succeeds only when the current
  bytes still hash to `expected.content_id` (or the file is still absent for
  `absent`), is applied through a same-directory temporary file, `fsync`, and
  `rename`, and preserves the existing permission bits. Identical content is a
  reasoned `no_op` that writes nothing.
- Editable scope is explicit: regular UTF-8 files up to 1 MiB without NUL
  bytes, reached without traversing any symlink, never `.git` (file or
  directory) at any depth, never anything under top-level `.wt/`, and never a
  file with any execute bit. Everything else under the root is editable text.
- Writer exclusion is published eligibility, not a hope: a worktree write is
  refused while the owner holds a live managed process group for the change
  that owns the worktree, while that change is in an active display status,
  or while the repository root mutation guard is held; a change-scope write is
  refused while the change is in an active status or the root is holding an
  unresolved merge or resolve. The read resource reports `editable` and
  `edit_blocked_reason` from the same classifier the command uses.
- A stale write is refused with the typed code `text_stale` and a result that
  carries the current `content_id`; nothing is written and the owner never
  asks the client to discard anything, so the Web client can keep its buffer
  and re-read. Saving never commits, stages, or pushes; Git dirty state follows
  from the file write alone, and the worktree observation is refreshed after a
  successful write so `dirty` reflects it on the next read.

Capabilities gain a `text_edit` block, `actions` are untouched, the generated
OpenAPI document publishes the two routes, two command variants, the
`text_write` result kind, and the two new error codes. `cflx client` and
`cflx client mcp` gain no verb.

## Acceptance Criteria

1. A read of an editable file returns its bytes as UTF-8 `content`, a
   `content_id` equal to `sha256:` of those bytes, `size_bytes`, the
   octal `mode`, `editable: true`, and the limits; a read advances no revision
   and creates no command record.
2. A write whose `expected.content_id` equals the current file identity
   replaces the bytes atomically, keeps the previous permission bits, returns a
   `text_write` result with `outcome: written`, `content_id_before`,
   `content_id_after`, and `bytes_written`, and the next `git status` of that
   worktree shows the path as modified; a write whose content equals the file
   settles `no_op` with `outcome: unchanged` and leaves mtime and inode intact.
3. A write whose `expected` no longer matches — the file changed, was
   removed, or exists when `absent` was expected — fails with `text_stale`,
   writes nothing, leaves no temporary file in the directory, and the record's
   result carries the current `content_id` (or `null` when absent).
4. Every containment refusal is typed and effect-free: a lexical violation
   (absolute path, `..`, empty or `.` component, backslash, over 1024 bytes,
   non-UTF-8) is `validation_failed`; a path that traverses a symlink at any
   component, names a non-regular file, names `.git` as a file or a directory,
   lies under top-level `.wt/`, has any execute bit, exceeds 1 MiB, or is not
   valid UTF-8 / contains NUL is `text_not_editable` with a stable `reason`.
   A symlink substituted after a successful read is still refused at write
   time because the write path re-walks every component with `O_NOFOLLOW`.
5. Writer exclusion holds: the worktree of a change in an active status or
   with a live managed process, and any target while the root guard is held,
   refuse with `target_ineligible` and a stable reason, and the read resource
   already reported `editable: false` with the same reason.
6. Only the addressed file changes. A real-Git regression proves that after a
   write the only differing path between the two observations of the
   worktree is the written file, no other worktree or the project root is
   touched, no commit is created, and the temporary file name never survives.
7. Compatibility: the surface is additive. An owner that predates it serves
   every existing route and command unchanged; a client built with this
   contract decodes its capabilities with `text_edit` absent, never as
   available. A request naming either new command against such an owner is
   `validation_failed` before any effect. `cflx client` verbs are unchanged
   and pass their existing suites.

## Explicit Completion Conditions

The real-Git regressions declared in `design.md` run under
`remote_text_edit` with nonzero passed counts in both declared gates; a
zero-test selection is a failure. Both the read-only and the write path are
exercised against a disposable repository with a linked worktree whose `.git`
is a file. `docs/guides/USAGE.md` documents the surface, its limits, and the
race boundary. Proposal validation is not implementation evidence; every task
remains unchecked until implemented and verified in a separately authorized
execution.

## Scope / Preserved Contracts

Implementation touches `src/web/remote_control_api/` (new `text_edit.rs`
module, `dto.rs`, `reads.rs`, `mod.rs` routes, `executor.rs` dispatch,
`worktrees.rs` capability wiring), `src/web/openapi.rs`, `src/web/state.rs`
for the eligibility facts the classifier needs, their focused tests, and the
usage guide. `delete_worktree`, `merge_worktree`, `create_worktree`,
`refresh_proposal`, every read resource, authentication, revision fencing,
idempotency, and the TUI `e` key are unchanged. No new dependency: hashing
uses the existing `sha2` crate. No Conflux-owned file is left in a target
worktree: the only transient artifact is the same-directory temporary file
that the rename consumes, and it is removed on every failure path.

The tracked `.pre-commit-config.yaml` scopes Rust checks by staged paths; the
declared focused gates stay explicit and no broad build is added as a task.

## Out of Scope / Rollout Handoff

Not included: binary or non-UTF-8 content, directory listing, file creation
outside the two roots, rename or delete of files, any automatic commit,
stage, or push, conflict merging on stale writes, a Web editor UI, Server
relay code, and new `cflx client` or MCP verbs. The Server owns project
provisioning (clone, pull, push) and the stale-text client behavior; cflx
owns only the bounded file effect defined here. Running owners are not
restarted or patched by this change; the feature exists only in a binary
built from it.
