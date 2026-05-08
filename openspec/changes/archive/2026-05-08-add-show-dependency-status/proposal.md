---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/cli/spec.md
  - openspec/changes/archive/2026-05-08-add-openspec-list-dependency-status
---

# Add dependency status to openspec show

**Change Type**: implementation

## Problem/Context

`cflx openspec list` now surfaces proposal dependencies with workspace-local status labels, but `cflx openspec show <change-id>` only displays the proposal body, task progress, design, and spec deltas. Operators inspecting one change cannot see the same normalized dependency summary unless they switch back to list output or manually inspect `proposal.md` metadata.

The existing list behavior already parses dependencies and classifies them as `done`, `running`, `pending`, or `missing` from workspace-local repository evidence. The show command should reuse that behavior so detailed inspection and list inspection tell the same story.

## Proposed Solution

Extend `cflx openspec show <change-id>` so both human-readable and JSON output include parsed proposal dependencies and their workspace-local statuses for active changes.

- Human-readable output renders `Dependencies: <id> [<status>]` when dependencies exist.
- JSON output includes machine-readable dependency status entries.
- The same classification rules used by `cflx openspec list` are reused for consistency.
- Changes without dependencies omit the human-readable `Dependencies:` line and expose an empty or absent JSON dependency collection according to the implementation's existing JSON style.
- `--deltas-only` remains focused on spec deltas and does not render dependency details.

## Acceptance Criteria

- `cflx openspec show <active-change>` displays a `Dependencies:` line for active changes that declare dependencies.
- Each displayed dependency uses the same `<dependency-id> [done|running|pending|missing]` labels as `cflx openspec list`.
- `cflx openspec show --json <active-change>` exposes dependency statuses as structured JSON data, not only as proposal text.
- `cflx openspec show <independent-change>` does not print an empty `Dependencies:` line.
- `cflx openspec show --deltas-only <change-id>` remains limited to spec delta details.
- Dependency classification remains derived only from workspace-local evidence and does not depend on logs or out-of-worktree durable state.

## Explicit Completion Conditions

- `src/openspec_cmd.rs` carries dependency IDs and status entries through `OpenSpecManager::show_change()` into `ShowInfo` for non-archived, non-deltas-only show results.
- Human-readable `cmd_show()` output prints dependency status summaries using the same labels as `render_changes_output()`.
- JSON `cmd_show()` output includes structured dependency status objects that downstream tools can parse.
- Unit tests under `src/openspec_cmd.rs` cover pending, running, done, missing, independent, JSON, and deltas-only show behavior.
- `cargo test openspec_cmd --lib` passes.
- `cflx openspec validate add-show-dependency-status --strict --evidence warn` passes without evidence warnings.

## Out of Scope

- Changing dependency classification semantics.
- Adding dependencies to `cflx openspec list --specs`.
- Adding or changing orchestration scheduling behavior.
- Surfacing dependency status in the TUI or Web UI.
