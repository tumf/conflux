---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/orchestration/state.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/tui/orchestrator.rs
  - src/tui/command_handlers.rs
---

# Fix ResolveWait dirty-clean transition

**Change Type**: implementation

## Problem / Context

Parallel merge retry state can strand a change in `resolve pending` when repository cleanliness changes around scheduler-owned merge retry.

Observed behavior:

- When the base repository is dirty and no change is actively `resolving`, a `resolve pending` change does not reliably transition back to `merge wait`.
- After the repository becomes clean again, a `resolve pending` change does not reliably promote to `resolving`.

This breaks the scheduler-owned `M` key contract: `M` records reducer-visible retry intent, and the parallel scheduler is responsible for promoting, retrying, or demoting that intent based on observable repository/workspace state.

The change must follow `openspec/CONSTITUTION.md`: workflow-control decisions must remain derivable from workspace file state, workspace git state, and base-branch comparison. No out-of-worktree durable state may become authoritative for retry routing.

## Proposed Solution

Tighten the reducer and scheduler behavior for `ResolveWait` retries:

- Treat dirty base with no active base-mutating lane occupant as manual intervention, demoting the change from `ResolveWait` to `MergeWait` via authoritative reducer state.
- Treat clean base with a free base-mutating lane as a retry-ready condition, promoting exactly one reducer-owned `ResolveWait` change to `Resolving` during scheduler evaluation.
- Ensure dirty-to-clean transitions wake or re-evaluate scheduler-owned retry work without requiring another `M` keypress.
- Preserve existing protections: `ChangesRefreshed` alone must not regress `ResolveWait`; terminal merged/rejected rows must not resurrect; only one base-mutating resolve/reject lane occupant may exist.

## Acceptance Criteria

- A `ResolveWait` change retried while the base repository is dirty and no other change is actively `Resolving` or `Rejecting` becomes `MergeWait` and is removed from reducer-owned resolve-wait queues.
- A `ResolveWait` change observed while the base repository is clean and the base-mutating lane is free is promoted to `Resolving` by scheduler-owned execution.
- Dirty-to-clean cleanup of the base repository causes pending retry work to progress without a second user keypress.
- `ChangesRefreshed` workspace observations alone do not convert `ResolveWait` to `MergeWait`; only concrete retry/deferred evidence may do that.
- At most one change is `Resolving` or `Rejecting` at any time according to reducer invariants.
- Terminal `Merged` and `Rejected` changes are not reintroduced into `ResolveWait`, `MergeWait`, or `Resolving` by stale refresh or retry paths.
- TUI display status remains consistent with `OrchestratorState::all_display_statuses()` for `resolve pending`, `merge wait`, and `resolving` rows.

## Explicit Completion Conditions

- `src/orchestration/state.rs` contains reducer coverage proving `ResolveWait + MergeDeferred(auto_resumable=false)` transitions to `MergeWait`, and clean promotion sets exactly one waiter to `Resolving`.
- `src/parallel/queue_state.rs` and/or related scheduler code contains coverage proving dirty retry demotion and dirty-to-clean retry promotion are scheduler-owned and do not depend on out-of-worktree durable state.
- `src/parallel/merge.rs` dirty base classification remains explicit and does not depend on parsing human-readable deferred reason strings.
- TUI command/runner coverage proves `M` remains intent-only and display sync follows reducer state after demotion or promotion.
- Relevant tests pass with the repository's Rust test command, and OpenSpec strict/evidence validation passes for this change.

## Out of Scope

- Changing the meaning of manual conflict resolution or adding a new UI control.
- Persisting retry state outside the repository/workspace as authoritative workflow state.
- Changing merge conflict resolution strategy, hook behavior, or archive completion semantics beyond the dirty-clean `ResolveWait` transition.
- Rewriting parallel scheduling broadly outside the base-mutating lane retry path.
