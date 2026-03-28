# Fix: MergeWait change regresses from ResolveWait when queued during active resolve

## Problem / Context

When a user presses `M` on a `MergeWait` change while another change is already resolving, the TUI correctly transitions the change to `ResolveWait` and adds it to the local resolve queue. However, within seconds, the change regresses back to `MergeWait`.

**Root cause**: `resolve_merge()` in `src/tui/state.rs:843-858` updates TUI-local state (`queue_status` and `resolve_queue`) but does **not** sync the intent to the shared orchestrator reducer (`shared_orchestrator_state`). On the next `ChangesRefreshed` event, `apply_display_statuses_from_reducer()` overwrites the TUI status with the reducer's stale `"merge wait"`, undoing the user's action.

An established pattern for this sync already exists: `start_processing()` (line 1094-1101) applies `ReducerCommand::AddToQueue` to the shared reducer to prevent regression. The same pattern must be applied to `resolve_merge()`.

## Proposed Solution

In `resolve_merge()`, when `is_resolving == true` (queued path), apply `ReducerCommand::ResolveMerge` to `shared_orchestrator_state` immediately after adding to the local resolve queue. This ensures the reducer transitions the change to `WaitState::ResolveWait`, which is protected from `apply_observation(WorkspaceArchived)` overwrite (line 678).

## Acceptance Criteria

1. Press `M` on a `MergeWait` change while another change is resolving; the change must remain in `ResolveWait` ("resolve pending") across subsequent `ChangesRefreshed` events until resolve actually starts or completes.
2. The change must eventually be resolved when its turn comes in the resolve queue.
3. No regression: `MergeWait` changes that have NOT been queued for resolve must still display as `MergeWait`.
4. Existing tests (`test_resolve_merge_queues_when_resolving`, `test_resolve_merge_starts_immediately_when_not_resolving`) must continue to pass.

## Out of Scope

- Changes to the resolve execution pipeline itself.
- Changes to the `apply_observation` reconciliation logic (it already correctly protects `ResolveWait`).
