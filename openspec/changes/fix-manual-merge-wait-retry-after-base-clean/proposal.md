---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-merge/spec.md
  - src/tui/command_handlers.rs
  - src/tui/state/event_handlers/errors.rs
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - src/orchestration/state.rs
---

# Fix Manual Merge-Wait Retry After Base Clean

**Change Type**: implementation

## Problem / Context

A TUI manual merge retry can get stranded after the following sequence:

1. A change reaches post-archive merge handling.
2. The base repository is dirty, so the merge attempt is deferred with `MergeDeferred(auto_resumable=false)`.
3. The row correctly returns to `merge wait` because manual action is required.
4. The user cleans the base repository and presses `M` again.
5. The UI logs that retry intent was scheduled, but resolve/merge work does not start.

Existing canonical specs already require `ResolveMerge` / `MergeWait` retry to be reducer-owned scheduler intent. The observed behavior violates that contract because an explicit retry after the manual blocker is removed should be consumed by the scheduler and should reach the merge attempt path.

This proposal follows `openspec/CONSTITUTION.md`: workflow-control decisions remain derivable from workspace file state, workspace git state, and base-branch tree comparison. It must not introduce new durable out-of-worktree workflow state.

## Proposed Solution

Ensure that explicit manual retry from `merge wait` after the base repository becomes clean always creates scheduler-consumable retry work and wakes the scheduler path that can consume it.

The implementation should verify and, if needed, repair the handoff across these boundaries:

- TUI `M` handling must apply `ReducerCommand::ResolveMerge` to the shared reducer and reject only truly stale/final states.
- Scheduler notification or scheduler startup must cause lane-wait evaluation even when no ordinary queued apply candidates remain.
- Executor retry dedupe state must not suppress an explicit retry that follows a prior dirty-base manual deferral.
- Retry dispatch must reach `attempt_merge()` when the archive-complete workspace still exists and the base repository is clean.
- If the retry remains blocked, the user must receive reducer/TUI/log evidence explaining the current blocker instead of silent inactivity.

## Acceptance Criteria

- Dirty base during manual post-archive merge retry still returns the change to visible `merge wait` with `MergeDeferred(auto_resumable=false)`.
- After the base repository is cleaned, pressing `M` on that same `merge wait` row is accepted by the reducer as `ResolveWait`.
- A live scheduler notified by the TUI consumes the reducer-owned lane waiter even when the ordinary queued list is empty.
- A newly started manual resolve scheduler consumes the same shared reducer-owned lane waiter when the scheduler was previously stopped.
- Stale executor-local dispatch dedupe or dirty-state tracking cannot suppress a new explicit retry after manual deferral.
- When no blocker remains, the retry reaches the merge attempt path and can transition the change to `merged`.
- When a blocker remains, the change returns to the correct wait state and emits enough log/reducer evidence to diagnose why no merge started.

## Explicit Completion Conditions

This proposal is complete when repository-verifiable evidence shows:

- `src/orchestration/state.rs` accepts explicit retry from manual `MergeWait` into reducer-owned `ResolveWait` for archive-complete, not-yet-merged workspaces without reintroducing ordinary queued apply work.
- `src/tui/command_handlers.rs` and the parallel scheduler path ensure `Scheduled merge-wait retry intent ...; notified existing scheduler` is followed by lane-wait retry evaluation, not silent idle/drained behavior.
- `src/parallel/queue_state.rs` cannot use stale `last_dispatched_resolve_wait_changes`, `last_resolve_wait_base_dirty`, or related local caches to suppress a user-initiated retry after `MergeDeferred(auto_resumable=false)`.
- Regression tests cover dirty-base deferral, base-clean explicit retry, scheduler-alive notify, scheduler-stopped startup, and dedupe reset behavior.
- `cargo test` or narrower documented Rust test commands pass for the affected reducer, TUI command handler, and parallel executor tests.
- `cflx openspec validate fix-manual-merge-wait-retry-after-base-clean --strict --evidence warn` passes.

## Out of Scope

- Changing the constitution or introducing durable out-of-worktree workflow-control state.
- Reworking the entire post-archive merge lifecycle beyond the manual retry wake/dispatch bug.
- Changing final-state semantics for already merged, rejected, or otherwise terminal changes.
- Changing user-facing key bindings or adding new TUI commands.
