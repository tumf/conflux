---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/runner.rs
  - src/tui/state.rs
  - src/tui/state/selection_logic.rs
  - src/tui/render.rs
  - src/orchestration/state.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/tui-state-management/spec.md
---

# Fix TUI Running Reducer Sync

**Change Type**: implementation

## Problem / Context

TUI Running mode regressed after the display state was made reducer-snapshot-driven. Operators report that changes added while the TUI is already running can no longer be controlled reliably with `Space` or `x`, and the header no longer shows the expected `[Running:N]` in-flight count.

The current TUI runner only applies a subset of `ExecutionEvent`s to the shared reducer before syncing `display_status_cache` back into `AppState`. Lifecycle events such as `ProcessingStarted`, `ApplyStarted`, `AcceptanceStarted`, `ArchiveStarted`, and their completion/failure counterparts are handled by the reducer implementation, but they are not included in the TUI runner's reducer-sync gate. This can leave the reducer display snapshot stale or incomplete, causing `display_status_cache` to regress away from active/queued states on refresh.

This proposal does not change the constitutional rule that workflow control state must remain workspace-derivable. The reducer/TUI state remains runtime/UI observability and control intent, not durable workspace routing authority.

## Proposed Solution

Update TUI event processing so the shared reducer receives the full set of execution lifecycle events required to derive accurate display status for Running mode. The TUI should continue to apply the reducer display snapshot to `AppState`, but only after the reducer has observed the same lifecycle transitions that the local TUI event handlers already process.

Specifically, the implementation should ensure:

- Running lifecycle starts (`ProcessingStarted`, `ApplyStarted`, `AcceptanceStarted`, `ArchiveStarted`, `ResolveStarted`) update reducer-visible activity before display sync.
- Lifecycle completions and failures update reducer-visible activity/terminal state before display sync.
- Running-mode queue controls (`Space` and `x`) continue to see accurate `not queued`, `queued`, `error`, and active statuses after refresh.
- Header in-flight count remains based on active display statuses and does not disappear while apply/accept/archive/resolve work is active.

## Acceptance Criteria

- While TUI is Running, a newly discovered `not queued` change can be marked with `Space`, emits `TuiCommand::AddToQueue`, and becomes reducer-visible `queued` without being reverted by subsequent `ChangesRefreshed` display sync.
- While TUI is Running, a queued but not-yet-active change can be unmarked with `Space`, emits `TuiCommand::RemoveFromQueue`, and is excluded from later dispatch.
- While TUI is Running, `x` can bulk mark/unmark eligible non-active rows according to existing single-row queue semantics, while active rows remain protected from bulk toggling.
- When one or more changes are in `applying`, `accepting`, `archiving`, or `resolving`, the TUI header displays `Running <count>` and excludes merely queued rows from the count.
- `ChangesRefreshed` after lifecycle start events does not regress active rows to `queued` or `not queued` while work is still active.

## Explicit Completion Conditions

The change is complete when repository evidence shows that:

- `src/tui/runner.rs` or its extracted helper applies all reducer-supported lifecycle events needed by Running mode display/status derivation before calling `AppState::apply_display_statuses_from_reducer`.
- Unit tests cover the reducer-sync event predicate or equivalent runner helper so future lifecycle events cannot silently fall out of TUI display sync.
- TUI state/render tests prove Running queue controls and header count survive reducer display sync plus `ChangesRefreshed` refresh ordering.
- The default Rust test/lint/typecheck verification requested by the repository is run or, if unavailable/too slow, the exact limitation is recorded with targeted test evidence.

## Out of Scope

- Changing durable workflow routing semantics or adding out-of-worktree durable workflow state.
- Replacing the reducer display snapshot architecture.
- Redesigning key bindings or the visual style of the TUI header.
- Implementing the previously specified active-change `Space` stop behavior beyond preserving existing behavior and avoiding new regressions.
