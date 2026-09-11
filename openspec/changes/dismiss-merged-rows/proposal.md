---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/state/processing_logic.rs
  - src/tui/key_handlers.rs
  - src/tui/render.rs
  - src/tui/types.rs
  - openspec/specs/tui-key-hints/spec.md
verifications:
  - id: merged-row-dismissal-tests
    requirement: "Individual and bulk dismissal immediately removes only merged rows from the current TUI session and refresh cannot restore them"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/state/processing_logic.rs
    evidence: "cargo test tui:: --lib"
    rerun: "cargo test tui:: --lib"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Dismiss reviewed merged rows from the TUI

**Change Type**: implementation

## Premise / Context

- The long-lived local TUI retains rows with display status `merged` after their active proposal disappears, so completed work accumulates in the Changes view.
- Retention currently happens in `src/tui/state/processing_logic.rs`; refresh deliberately preserves terminal rows and execution history.
- The Changes view currently assigns no action to `d` or `D`. The Worktrees view already owns those keys for worktree deletion, so view-specific routing remains unambiguous.
- The visual authority is the production TUI at revision `47821e12d80ed6afd2488fbcbe3855ef54c4df26`. This change preserves its row layout, colors, focus treatment, panels, and key-hint style.
- Dismissal is process-local presentation state. It must not alter OpenSpec archives, Git state, reducer state, logs, API snapshots, execution marks, queue state, or lifecycle decisions.

## Problem / Context

During continuous TUI operation, successfully merged rows remain visible indefinitely. Operators need to review those rows, then manually remove either the focused merged row or all merged rows in the current Changes-list projection without deleting repository evidence or changing orchestration state. In this change, "visible" means present in that projection regardless of scroll position.

A simple vector removal is insufficient because periodic refresh retains terminal rows and execution history. The TUI must remember dismissed IDs for the current process and suppress them on subsequent refreshes while leaving authoritative workflow state untouched.

## Proposed Solution

- Add `d` in the Changes view to request dismissal of the focused row when its current display status is exactly `merged`.
- Add `D` in the Changes view to request dismissal of all rows in the current Changes-list projection whose current display status is exactly `merged`.
- Both actions take effect immediately without a confirmation overlay.
- On the key press, record the affected IDs in a process-local dismissed-ID set, remove those rows from the local Changes projection, repair cursor/list selection, `NEW` count, known-ID bookkeeping, and selected-proposal log-filter state, then emit one informational TUI log entry.
- Later refreshes suppress only retained terminal/merged presentation state for dismissed IDs. If the catalog re-observes the same ID as an active non-terminal change, the TUI removes that ID from the dismissed set and renders it normally.
- A new TUI process starts with an empty dismissed-ID set and may show repository-derived merged rows again. No durable dismissal file is introduced.
- Render `d: dismiss` only when the focused row is `merged`; render `D: dismiss all merged` whenever at least one visible merged row exists. Existing hints and visual language remain unchanged.

## Acceptance Criteria

- Pressing `d` on a focused `merged` row immediately removes only that row from the current TUI session.
- Pressing `D` when projected merged rows exist immediately removes every `merged` row in the current Changes-list projection and no other row.
- Neither action opens a confirmation overlay or requires a second key press.
- `d` on a non-merged row and `D` with no visible merged rows are silent no-ops and expose no misleading hint.
- Dismissed rows stay absent as retained terminal/merged presentation state through refresh for the lifetime of the process; a later active non-terminal catalog change with the same ID is shown normally.
- Dismissal never deletes or edits `openspec/changes/archive/**`, Git refs/worktrees, logs, task files, reducer records, API/Web projection, queue membership, or execution marks.
- Cursor and list selection remain valid after removing the first, middle, last, all, or only row. A selected-proposal log filter is disabled if its target row is dismissed.
- Deterministic render tests prove the two context-aware hints while preserving the production TUI layout and selected-row styling.

## Explicit Completion Conditions

- `AppState` owns only process-local dismissal data; no new modal state, filesystem mutation, or reducer mutation is added.
- Key routing, projection filtering, cursor repair, and hints implement the specified behavior.
- Tests cover immediate individual and bulk dismissal, mixed statuses, repeated refresh, empty-list cursor state, and selected-log-filter cleanup.
- `cargo test tui:: --lib` passes.
- `cflx openspec validate dismiss-merged-rows --strict --evidence warn` and the archive gate pass.

## Out of Scope

- Automatically expiring merged rows.
- Persisting dismissed rows across TUI restarts.
- Dismissing `archived`, `pushed`, `rejected`, `error`, or waiting rows.
- Deleting archive directories, branches, worktrees, logs, or any other repository evidence.
- Adding equivalent dismissal controls to `/api/v2`, Web UI, or `conflux-server`.
- Changing merge, archive, run, retry, mark, queue, or lifecycle semantics.
