---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/orchestration/state.rs
  - src/parallel/dispatch.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/tui/runner.rs
  - src/tui/state/event_handlers/completion.rs
  - src/tui/state/event_handlers/errors.rs
  - src/web/state.rs
---

# Normalize base-mutating lane status transitions

**Change Type**: implementation

## Problem / Context

Parallel execution currently models archive-merge and rejection-review lifecycle transitions with overlapping local UI state, reducer state, and scheduler queues. Recent post-archive fixes define a reducer-owned decision table, but observed behavior still shows unstable or misleading statuses after archive completion: `archived`/`merged` vibration, `merge wait` without a preceding concrete manual blocker, and missing visible `resolving` during automatic merge handling.

The missing conceptual boundary is that `Resolving` and `Rejecting` are both base-mutating lane activities. They can dirty or otherwise mutate the base branch/workspace and therefore must be globally exclusive. Because queued work can wait for different next operations, Conflux needs both `resolve pending` for archive-merge retry intent and `reject pending` for rejection-review intent.

This proposal follows `openspec/CONSTITUTION.md`: queue/pending state may guide scheduling and display, but authoritative resume routing must remain derivable from workspace file state, workspace git state, and base-branch tree comparison. No out-of-worktree durable workflow-control state is introduced.

## Proposed Solution

Introduce a single reducer-owned base-mutating lane model:

- Active lane activities: `Resolving` and `Rejecting`.
- At most one non-terminal change may occupy the lane at any time.
- Archive-complete merge work waiting for that lane displays `resolve pending` and remains auto-resumable.
- Rejection-review work waiting for that lane displays `reject pending` and remains auto-resumable.
- Manual merge blockers continue to display `merge wait` and require explicit retry.

The implementation should add a distinct reducer wait state and queue membership for reject-review intent, synchronize TUI/Web display from reducer status for all relevant lifecycle events, and ensure scheduler promotion starts exactly one pending base-mutating operation when the lane clears.

## Acceptance Criteria

- When another change is actively `resolving` or `rejecting`, a newly archived change transitions to `resolve pending`, not `archived`, `merge wait`, or immediate parallel merge execution.
- When another change is actively `resolving` or `rejecting`, a change that needs rejection review transitions to `reject pending`, and rejection review does not start until the lane clears.
- `resolving` and `rejecting` are mutually exclusive across all non-terminal changes; the reducer and scheduler cannot expose both simultaneously.
- With no lane blocker and no manual merge blocker, an archived parallel change displays `resolving` while merge handling is active and `merged` after completion.
- `merge wait` is used only for concrete manual merge blockers or explicit manual retry state, not as the default archive-complete state.
- TUI and Web status displays derive from reducer-owned statuses for `resolve pending`, `reject pending`, `resolving`, `rejecting`, `merge wait`, and `merged`, without stale local handlers regressing terminal or active states.
- The change remains constitution-compliant: deleting logs/UI/cache state does not change workspace resume routing for apply/accept/archive/reject/resolve decisions.

## Explicit Completion Conditions

- `src/orchestration/state.rs` or equivalent reducer code contains a first-class reject-wait state that displays as `reject pending`, is distinct from `ResolveWait`, and exposes query/clear APIs for scheduler use.
- Reducer invariants or tests prove there is at most one active base-mutating lane occupant across `Resolving` and `Rejecting`.
- Parallel apply/rejection handoff code in `src/parallel/dispatch.rs` and scheduler code in `src/parallel/queue_state.rs` defer rejection review into `reject pending` when the lane is occupied, then promote it to `rejecting` when the lane clears.
- Parallel merge code in `src/parallel/merge.rs` and post-archive dispatch code continue to defer archive-merge work into `resolve pending` when the lane is occupied, including when the occupant is `Rejecting`.
- TUI synchronization in `src/tui/runner.rs` and local handlers in `src/tui/state/event_handlers/*.rs` cannot overwrite reducer-derived `resolving`, `rejecting`, `resolve pending`, `reject pending`, `merge wait`, or terminal `merged` with stale `archived`/`queued`/`not queued` states.
- Web state snapshots expose the reducer-derived `reject pending` and do not regress post-archive statuses.
- Targeted Rust tests cover reducer, scheduler, TUI, and Web display behavior; no default test taking over 1 second is left unmarked as heavy.

## Out of Scope

- Changing the semantics of confirmed terminal `rejected` or `merged` outcomes.
- Introducing out-of-worktree durable workflow-control state.
- Reworking unrelated dependency-blocked, stalled, or queued apply scheduling behavior.
- Changing the human-visible wording of existing `merge wait` or `resolve pending` beyond adding and wiring `reject pending`.
