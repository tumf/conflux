---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/tui-resolve/spec.md
  - src/tui/state/event_handlers/refresh.rs
  - src/tui/state.rs
  - src/tui/runner.rs
  - src/orchestration/state.rs
---

# Fix Manual Resolve Refresh Regression

**Change Type**: implementation

## Premise / Context

- Operators report that pressing `M` on a `merge wait` row briefly changes it to `resolve pending`, but a few seconds later the row returns to `merge wait` even when the workspace is clean and no other change is resolving.
- `ResolveMerge` already records reducer-owned `ResolveWait` when accepted by `src/orchestration/state.rs`.
- The TUI runner applies reducer display synchronization before local `ChangesRefreshed` handling, and `src/tui/state/event_handlers/refresh.rs` can then apply refresh-derived `merge_wait_ids` to the same row.
- Existing specs allow refresh-derived `merge wait` correction for stale display-only `resolve pending`, but manual reducer-owned `ResolveWait` must remain scheduler-consumable and must not be overwritten by display-only refresh evidence.
- The Constitution requires workflow-control inputs to remain workspace/git-derived and completion to be repository-verifiable; this change only corrects TUI display synchronization and scheduler intent preservation.

## Problem / Context

Periodic refresh detects archive-complete, not-yet-merged worktrees and emits `merge_wait_ids`. That evidence is valid for restoring a stale or display-only `merge wait` view, but it is not authoritative enough to override an already-accepted manual resolve retry intent. Because the current event handling order can apply reducer state first and refresh-local merge-wait evidence second, the visible row can oscillate from `resolve pending` back to `merge wait` before the scheduler consumes the retry.

This makes `M` appear ineffective and hides whether the scheduler accepted the manual retry. In the clean-workspace / no-other-resolving case, the expected behavior is for the row to remain `resolve pending` until it transitions through scheduler events to `resolving` / `merged`, or returns to `merge wait` only after explicit failure/defer evidence.

## Proposed Solution

Update TUI refresh display synchronization so refresh-derived `merge_wait_ids` does not downgrade rows whose reducer snapshot currently reports `resolve pending` for the same change.

The implementation should:

- Keep reducer-owned `ResolveWait` as the display authority for manual `M` retry intent.
- Continue using `merge_wait_ids` to correct stale display-only `resolve pending` rows when no reducer-owned resolve intent exists.
- Preserve terminal-row protections for stale refresh evidence.
- Preserve the existing separation between display synchronization and workflow control; refresh-derived display corrections must not enqueue, dispatch, archive, accept, or route work.

## Acceptance Criteria

- Pressing `M` on a visible `merge wait` row whose `ResolveMerge` is accepted by the reducer results in `resolve pending` that survives subsequent `ChangesRefreshed` events containing the same change in `merge_wait_ids`.
- A stale local `resolve pending` row without reducer-owned `ResolveWait` is still corrected to `merge wait` when refresh evidence reports the workspace as archive-complete and not merged.
- Terminal rows such as `merged` and `rejected` are not regressed to `merge wait` by stale `merge_wait_ids`.
- The scheduler-visible resolve retry lifecycle remains truthful: accepted manual retry intent is either consumed into `resolving` / `merged` or returns to `merge wait` only after explicit failure/defer evidence.
- The change does not introduce out-of-worktree durable workflow-control state or rely on logs/UI state for resume routing.

## Explicit Completion Conditions

- `src/tui/state/event_handlers/refresh.rs` or its caller has access to reducer-derived display state when applying refresh-derived merge-wait display corrections.
- Unit coverage proves reducer-owned `resolve pending` survives `ChangesRefreshed` with matching `merge_wait_ids` through the actual TUI refresh path, not only through direct reducer synchronization.
- Unit coverage proves stale display-only `resolve pending` still becomes `merge wait` under the same refresh evidence.
- Existing terminal merge-wait refresh regression coverage continues to pass.
- Focused cargo tests for TUI state/refresh and reducer resolve handling pass locally.

## Out of Scope

- Changing merge conflict resolution semantics.
- Changing scheduler dispatch policy for auto-resumable versus manual merge deferrals.
- Introducing durable retry state outside workspace/git-derived evidence.
- Reordering the entire TUI event loop unless the minimal display synchronization fix requires it.
