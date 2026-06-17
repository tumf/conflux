---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/key_handlers.rs
  - src/tui/state.rs
  - src/tui/command_handlers.rs
  - src/tui/state/event_handlers/errors.rs
  - src/orchestration/state.rs
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Fix TUI merge-wait retry after dirty workspace is cleaned

**Change Type**: implementation

## Premise / Context

- A user reported that a proposal in TUI `merge wait` correctly returns to `merge wait` after pressing `M` while the workspace/base is dirty, but does not merge after the workspace/base is cleaned and `M` is pressed again.
- The TUI `M` path flows through `handle_merge_key()` in `src/tui/key_handlers.rs`, `AppState::resolve_merge()` in `src/tui/state.rs`, and `TuiCommand::ResolveMerge` handling in `src/tui/command_handlers.rs`.
- Existing canonical specs already require manual merge retry to be reducer-owned scheduler work and require explicit retry after dirty manual deferral to be consumed by the active or newly-started scheduler.
- The likely fault area is the boundary between optimistic TUI display updates, reducer `ResolveMerge` acceptance, scheduler startup/notification, and executor-local retry dedupe after a dirty manual deferral.
- The Conflux constitution requires workflow state to be derivable from workspace/git/base evidence and requires completion to be repository-verifiable.

## Problem / Context

When a `merge wait` row is manually retried with `M` while the base/workspace is dirty, Conflux may correctly return the row to visible `merge wait`. After the user fixes the dirty state and presses `M` again, the retry must be treated as a fresh user intent and must reach scheduler-owned merge retry evaluation. The observed behavior is that the second `M` does not merge the proposal.

This is high priority because it traps users in an apparently actionable `merge wait` state: the UI accepts the retry key path, but no repository-visible merge result occurs after the manual blocker is resolved.

## Proposed Solution

Make manual `M` retry after dirty manual deferral reliably re-dispatch scheduler-owned retry work by tightening the TUI/reducer/scheduler contract:

1. Ensure one authoritative reducer state owns manual retry intent and is the same state consumed by scheduler/executor retry dispatch.
2. Ensure `M` after `MergeDeferred(auto_resumable=false)` and later clean workspace/base is accepted as fresh retry intent, even if the same change was previously dispatched and returned to `merge wait`.
3. Ensure command handling starts or wakes scheduler work whenever retry intent is accepted or already validly pending for the selected `merge wait` row.
4. Ensure executor-local dispatch snapshots, dirty-state transition tracking, or previous resolve-wait cache values cannot suppress an explicit clean retry.
5. Add regression coverage for dirty-first retry followed by clean retry and for terminal-state safety.

## Acceptance Criteria

- Pressing `M` on a `merge wait` row while no base-mutating operation is active and the base/workspace is dirty returns the row to `merge wait`, clears scheduler-owned `ResolveWait` membership, and emits visible blocker evidence.
- After the dirty state is cleaned, pressing `M` again on the same row records reducer-owned `ResolveWait` retry intent and starts or wakes scheduler-owned retry evaluation.
- The clean retry reaches the post-archive merge attempt path for the selected change without requiring another queued change or another keypress.
- If no blocker remains, the selected change can transition to `merged` through existing merge completion events.
- Stale retry dedupe, dirty-state tracking, and prior dispatch snapshots do not suppress the explicit clean retry.
- Stale `M` actions for final terminal states such as `merged` and `rejected` remain no-ops and do not reintroduce retry work.

## Explicit Completion Conditions

- `src/tui/state.rs`, `src/tui/command_handlers.rs`, `src/orchestration/state.rs`, and/or `src/parallel/queue_state.rs` are updated so manual retry intent is accepted and consumed consistently after dirty manual deferral.
- Regression tests cover the full dirty-first / clean-second retry lifecycle with repository-verifiable state transitions, not only display text.
- Tests prove that retry dispatch is not suppressed by previous dirty failure or executor-local dedupe.
- Tests prove that permanent terminal states are not retried by stale `M` input.
- `cflx openspec validate fix-tui-merge-wait-clean-retry --strict --evidence warn` passes.
- Rust verification commands for the touched modules pass.

## Out of Scope

- Changing the `M` key binding or TUI navigation model.
- Introducing durable workflow-control state outside workspace/git/base evidence.
- Redesigning the entire parallel scheduler or merge conflict resolution agent.
- Changing behavior for unrelated apply/accept/archive queue dispatch.
