# Design: TUI Running Reducer Sync

## Current Flow

The TUI runner receives `ExecutionEvent`s from the orchestrator. For selected events, it currently:

1. applies the event to the shared `OrchestratorState` reducer;
2. takes `all_display_statuses()` from the reducer;
3. calls `AppState::apply_display_statuses_from_reducer()`;
4. then lets the local `AppState::handle_orchestrator_event()` event handler run.

This makes the reducer display snapshot a primary source of displayed status. That architecture requires the reducer to observe the same lifecycle transitions that affect Running-mode queue controls and header count.

## Failure Mode

The reducer can derive active display statuses (`applying`, `accepting`, `archiving`, `resolving`) and queue/terminal statuses from lifecycle events, but the TUI runner currently gates only a subset of events into the reducer. If a lifecycle start event is omitted from the reducer sync path, the later reducer display snapshot may remain `queued` or `not queued`, and a refresh can overwrite the local active status.

This breaks two user-visible behaviors:

- `Space` / `x` decisions depend on `ChangeState.display_status_cache`, so stale status can make queue controls no-op or target the wrong rows.
- Header active count is computed from `display_status_cache` rows in `applying`/`accepting`/`archiving`/`resolving`, so stale status makes `[Running:N]` disappear.

## Design Direction

Keep the reducer display snapshot architecture, but make event sync complete and testable.

Implementation should prefer an explicit helper such as `should_apply_event_to_tui_reducer(event)` or an equivalent match that is unit-tested. The helper should include every `ExecutionEvent` variant that `OrchestratorState::apply_execution_event` uses to derive Running display state or terminal/queue state relevant to TUI controls.

The likely minimum event set includes:

- `ProcessingStarted`
- `ProcessingCompleted`
- `ProcessingError`
- `ApplyStarted`
- `ApplyCompleted`
- `ApplyFailed`
- `AcceptanceStarted`
- `AcceptanceCompleted`
- `AcceptanceFailed`
- `ArchiveStarted`
- `ArchiveFailed`
- `ChangeArchived`
- `ResolveStarted`
- `ResolveCompleted`
- `ResolveFailed`
- `MergeDeferred`
- `MergeCompleted`
- `WorkspaceStatusUpdated`
- rejection review lifecycle events already covered today where relevant
- stop/all-complete events when reducer display state requires them

The implementation should avoid broad catch-all syncing if any events are purely informational and would create noisy or misleading reducer state. However, any event that changes reducer display status, queue intent, current change, wait state, or terminal state must be included.

## Verification Strategy

- Unit-test the event sync helper directly so omitted lifecycle variants are visible as test failures.
- Use TUI state tests to reproduce the observed ordering: lifecycle event → reducer display sync → `ChangesRefreshed` → Space/x interaction.
- Use render-buffer tests for header text so the in-flight count is validated at the visible UI layer, not just by internal state.

## Constitution Alignment

The proposal changes transient TUI/reducer synchronization only. It does not introduce durable workflow-control inputs outside workspace file/git/base-branch state, and does not make UI state authoritative for resume routing, acceptance gating, archive routing, or next-action decisions.
