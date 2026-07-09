---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - skills/cflx-archive/SKILL.md
  - skills/cflx-archive/references/cflx-archive.md
  - src/execution/archive.rs
  - src/task_parser.rs
  - src/openspec_cmd.rs
  - src/openspec_cmd/archive.rs
---

# Change: Forbid manual archive fallback and validate archive layout

**Change Type**: implementation

## Problem / Context

A parallel archive run can pass acceptance, fail `cflx openspec archive <change> --yes`, and then have the archive agent manually create a fallback path with `mkdir`/`mv`.
The observed fallback created `openspec/changes/archive/2026-07-09/<change_id>/tasks.md`, which is not the canonical CLI archive layout.
Conflux then could not recognize the change as archived and later reported `tasks.md not found` even though the failure was actually invalid archive bookkeeping.

This violates truthful completion: repository-visible archive evidence must be valid before Conflux treats archive as successful or creates an archive-success commit.

## Proposed Solution

- Update bundled `cflx-archive` guidance to make `cflx openspec archive <change_id> --yes` the only supported archive mutation command, with `--skip-specs` allowed only for tooling-only changes.
- Explicitly prohibit manual archive directory creation or movement using `mkdir`, `mv`, `git mv`, or equivalent direct writes under `openspec/changes/archive/`.
- Require terminal failure when the CLI archive command fails; the archive agent must not hand-repair archive layout or create a success-style archive commit.
- Add archive layout validation in Conflux so nested archive paths such as `openspec/changes/archive/YYYY-MM-DD/<change_id>/` are detected as invalid archive layout.
- Strengthen archive completion and task progress resolution so invalid archive layout is reported explicitly instead of collapsing into generic `tasks.md not found` or success-like archive completion.

## Acceptance Criteria

- Archive agents are instructed to stop on `cflx openspec archive <change_id> --yes` failure rather than manually moving directories.
- Archive guidance does not document or permit direct `mkdir`/`mv`/`git mv` archive fallback.
- Conflux accepts canonical dated archive entries at `openspec/changes/archive/YYYY-MM-DD-<change_id>/` and existing legacy direct entries at `openspec/changes/archive/<change_id>/` for read compatibility.
- Conflux rejects nested archive entries such as `openspec/changes/archive/YYYY-MM-DD/<change_id>/` with an explicit invalid-layout diagnostic that names the offending path and expected layout.
- Archive completion cannot succeed merely because `openspec/changes/<change_id>` is absent; it also requires a valid archive entry and no invalid matching nested entry.
- Task progress lookup reports invalid archive layout when matching nested archive evidence exists, rather than reporting only `tasks.md not found`.
- No archive-success commit/finalization path proceeds from invalid archive layout.

## Explicit Completion Conditions

- `skills/cflx-archive/SKILL.md` and `skills/cflx-archive/references/cflx-archive.md` contain clear terminal-failure guidance for CLI archive failure and explicit direct-move prohibitions.
- `src/execution/archive.rs` validates archive completion against valid archive entries and rejects nested layout evidence.
- `src/task_parser.rs` or a shared archive helper surfaces invalid archive layout during archived task lookup.
- `src/openspec_cmd.rs`/archive lookup paths use the same valid-layout rules where archived changes are resolved.
- Tests fail on the observed bad layout `openspec/changes/archive/2026-07-09/<change_id>/tasks.md` and pass for `openspec/changes/archive/2026-07-09-<change_id>/tasks.md`.
- `cflx openspec validate forbid-manual-archive-fallback --strict` passes.

## Out of Scope

- Migrating existing valid legacy direct archive directories.
- Changing the canonical `cflx openspec archive` destination format.
- Adding an automatic repair command for invalid archive layouts.
- Changing acceptance verdict semantics.
