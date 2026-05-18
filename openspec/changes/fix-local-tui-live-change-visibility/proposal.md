---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/runner.rs
  - src/tui/render.rs
  - src/tui/state/processing_logic.rs
  - src/openspec.rs
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/tui-state/spec.md
---

# Fix local TUI live change visibility

**Change Type**: implementation

## Problem/Context

Local TUI mode is expected to notice valid `openspec/changes/<id>` directories added while the TUI is already running. The refresh path still polls the change list and appends new rows, but users can miss newly discovered changes in Running mode because the changes list may be vertically constrained by the logs panel and newly appended rows remain below the current cursor viewport.

The regression became visible after recent Running-mode log flex layout changes. Select mode has a footer `New: N` indicator, but Running mode does not surface the same discovery signal. As a result, a valid change can be present in `AppState::changes` with `is_new = true` while the visible screen does not show either the row or a new-change count.

There is also a related robustness issue: the local refresh task captures `repo_root`, but the active change scan still calls cwd-relative helpers. Refresh should read from the captured repository root so long-lived TUI sessions do not depend on ambient process cwd for change discovery.

This proposal is constrained by `openspec/CONSTITUTION.md`: any new visibility/log state is observability-only and MUST NOT become workflow-control input.

## Proposed Solution

- Make local TUI auto-refresh read active and rejected changes from the captured `repo_root`, not from ambient cwd.
- Preserve the existing behavior that new active changes are appended as unselected `not queued` rows with `is_new = true`.
- Add an explicit Running-mode visibility signal for newly discovered changes so users can see that new work arrived even when the new row is outside the current viewport.
- Add an observability log for newly detected active changes, without using that log for scheduling, resume routing, archive routing, acceptance, or next-action decisions.
- Keep cursor position stable by default so live discovery does not steal user focus while they operate the TUI.

## Acceptance Criteria

- Local TUI refresh discovers a valid new active change added under `openspec/changes/<id>` after startup and keeps it in the TUI changes list as a `not queued` row with `is_new = true`.
- Running mode shows a user-visible new-change signal when `new_change_count > 0`, even when the new row is appended outside the visible changes-list viewport.
- Newly detected active changes produce a TUI log entry identifying the change id, and this log remains observability-only.
- The local refresh task scans active changes and rejected marker rows from the captured repository root instead of relying on process cwd.
- Rejected marker rows remain visible as read-only `rejected` rows and do not receive `NEW` badges.
- Cursor position is not automatically moved solely because a new change was discovered.

## Explicit Completion Conditions

- `src/tui/runner.rs` uses a repo-root-based change listing path for local auto-refresh.
- `src/openspec.rs` exposes repo-root-based rejected change listing or equivalent support so TUI refresh can avoid cwd-relative rejected scans.
- `src/tui/state/processing_logic.rs` or its call path can update changes using both active and rejected lists derived from the same captured root.
- `src/tui/render.rs` displays `New: N` or an equivalent signal in Running mode when new active changes are present, including logs-panel-enabled layouts.
- Tests prove that local refresh/change-state update detects new active changes, keeps rejected rows non-new, renders a Running-mode new-change signal, and does not depend on cwd for repo-root-based listing.
- Validation passes with `cargo test` for the affected TUI/OpenSpec modules and final OpenSpec validation.

## Out of Scope

- Remote TUI mode local filesystem discovery. Remote mode remains driven by server/WebSocket state.
- Automatically queueing, selecting, dispatching, or otherwise changing execution intent for newly detected changes.
- Reworking the Running-mode logs panel layout beyond the visibility signal needed for this bug.
- Persisting new discovery state outside the workspace or using logs/UI state as workflow-control input.
