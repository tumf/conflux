# Design: Fix Manual Resolve Refresh Regression

## Current Flow

1. The user presses `M` on a row displayed as `merge wait`.
2. `AppState::resolve_merge()` updates the local row to `resolve pending` and attempts to synchronize `ReducerCommand::ResolveMerge` into shared reducer state.
3. `handle_tui_command()` applies the reducer command again and notifies or starts the scheduler only when the reducer accepts the intent.
4. The background refresh loop emits `ChangesRefreshed` with `merge_wait_ids` when the workspace remains archived but not merged.
5. The TUI runner applies reducer display state, then handles the local `ChangesRefreshed` event. The local refresh handler can set any matching non-terminal row to `merge wait`.

The problematic edge is step 5: refresh evidence is valid for stale display correction, but it can arrive before scheduler consumption and overwrite an accepted `ResolveWait` display.

## Desired Authority Order

For display synchronization only:

1. Terminal reducer statuses remain highest priority.
2. Active scheduler/reducer states such as `resolving` and reducer-owned `resolve pending` take priority over refresh-derived `merge_wait_ids`.
3. Refresh-derived `merge_wait_ids` may correct rows that have no reducer-owned `ResolveWait` or active/terminal lifecycle state.
4. Refresh-derived corrections must not dispatch work or influence resume routing.

This keeps the Constitution's workspace-local workflow-state rule intact because no new durable workflow state is introduced. The reducer and scheduler remain responsible for actual retry ownership; the TUI only avoids displaying weaker refresh evidence over stronger accepted intent.

## Implementation Approach

Prefer a minimal change in the TUI display synchronization layer:

- Make `handle_changes_refreshed()` / `apply_refresh_merge_wait_status()` aware of the current reducer display snapshot, or pass enough information from the runner/state layer to know whether a row is reducer-owned `resolve pending`.
- Skip `merge wait` overwrite when the reducer display for that change is `resolve pending`.
- Keep existing terminal guards.
- Do not use local TUI-only flags as scheduler dispatch inputs.

An alternative is to reorder the runner so local refresh handling happens before reducer display synchronization. That may be valid, but it is broader and should only be used if the minimal status-aware refresh fix creates worse coupling.

## Verification Strategy

- Unit tests should exercise the actual TUI refresh handler, not only `OrchestratorState::apply_execution_event()`.
- Regression coverage should include both sides of the distinction:
  - accepted reducer `ResolveWait` plus `merge_wait_ids` remains `resolve pending`
  - display-only stale `resolve pending` plus `merge_wait_ids` becomes `merge wait`
- Existing terminal refresh tests protect against stale `merge_wait_ids` regressing `merged` / `rejected` rows.
