# Design: Local TUI live change visibility

## Current Flow

Local TUI startup captures the repository root and initializes `AppState` from `openspec::list_changes_native()`. A background auto-refresh task later polls every five seconds and sends `ChangesRefreshed` to the TUI event loop.

The TUI state update path already supports newly discovered changes:

1. active changes are listed,
2. rejected marker rows are listed separately,
3. unknown ids become `new_ids`,
4. active new rows are appended with `is_new = true`,
5. rejected new rows are appended as `rejected` without the `NEW` badge.

The user-visible failure is mostly presentation: Running mode can reserve a fixed 20-row logs panel, append new changes below the cursor viewport, and omit the Select-mode `New: N` footer signal.

## Proposed Runtime Shape

### Explicit repository-root refresh

The local refresh task should use the captured `repo_root` for active and rejected listing. This avoids dependence on ambient process cwd in a long-lived TUI session.

Preferred shape:

- keep `openspec::list_changes_native_from(&refresh_repo_root)` as the active listing source,
- add `openspec::list_rejected_changes_native_from(&refresh_repo_root)`,
- pass the rejected list into the TUI update path that already accepts injected rejected changes for tests, or introduce a small public/internal helper that does the same.

This is a correctness guard. It does not change workflow-control state and does not persist anything outside the workspace.

### Visibility without focus stealing

Newly detected active changes should not automatically move the cursor. A running user may be inspecting or controlling an active row. Moving focus would make keyboard actions surprising.

Instead, Running mode should present a stable signal when `new_change_count > 0`. Acceptable UI locations include:

- changes list title,
- status panel,
- header.

The signal should be available even when logs are enabled and the appended row is outside the visible list viewport.

### Observability log

When active changes are newly detected, TUI should add an informational log such as `Detected new change: <id>`. This gives users a scrollback trail when the row is not immediately visible.

The log must remain observability-only. It must not enqueue, select, dispatch, route, accept, archive, or otherwise control workflow behavior.

## Trade-offs

- Auto-scrolling to new rows would make the change obvious, but it risks stealing focus during active operation. This proposal chooses a signal-plus-log approach.
- Expanding the changes list at the expense of logs could also expose appended rows, but it would partially undo the recent log flex layout. This proposal keeps the layout and fixes discoverability.
- Root-based listing requires a small helper addition for rejected rows, but prevents a fragile cwd dependency in the refresh path.

## Verification Strategy

- Unit tests cover explicit-base listing for active and rejected rows with cwd intentionally pointed elsewhere.
- State/update tests cover new active row flags, rejected row behavior, log emission, and cursor stability.
- Render tests cover Running mode with logs enabled and many rows so the new row may be off-screen but the new-change signal remains visible.
- Manual dogfood covers the live TUI sequence that triggered the report.
