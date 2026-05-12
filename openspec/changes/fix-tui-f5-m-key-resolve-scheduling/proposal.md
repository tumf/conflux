---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/key_handlers.rs
  - src/tui/state.rs
  - src/tui/command_handlers.rs
  - src/tui/render.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/tui-key-hints/spec.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/parallel-merge/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: Fix TUI F5 and M-key Resolve Scheduling

**Change Type**: implementation

## Problem / Context

The TUI currently mixes two different controls around `MergeWait` rows:

- `F5` is intended to be a cursor-independent orchestration control for starting, resuming, or retrying marked runnable work.
- `M` is intended to be the cursor-local action for a `MergeWait` row, registering scheduler-owned merge retry intent.

A historical spec change incorrectly introduced a `F5` path that resolves the cursor's `MergeWait` row. This makes `F5` depend on cursor position and overlaps with `M`.

The `M` path also needs sharper rules for `resolve pending` classification. A workspace/base can be dirty while another resolve or base-mutating operation is active, so dirty state must not be classified before checking whether resolve/base-mutating work is already in progress.

The Conflux constitution constrains this change: workflow-control decisions must remain derivable from workspace/git/base-tree evidence and in-memory scheduler/reducer state must not become durable out-of-worktree workflow control.

## Proposed Solution

- Restore `F5` as a cursor-independent orchestration control.
  - `F5` must not inspect the cursor row to dispatch `ResolveMerge`.
  - `F5` must not be blocked by an unrelated active resolve.
- Keep `M` as the only Changes-view key that registers cursor-local `MergeWait` retry intent.
  - `M` registers reducer-owned scheduler intent and does not directly execute merge/resolve outside the scheduler loop.
  - `M` may display `resolve pending` only while scheduler-owned retry intent is accepted and pending.
- Define the retry classification order explicitly:
  1. Check active resolve/base-mutating lane occupancy.
  2. Only if no such operation is active, check workspace/base dirty state.
- Preserve scheduler-owned retry behavior while the orchestrator is running.
  - If one or more `ResolveWait` rows exist, no resolve is active, and retry preconditions are clean, the scheduler starts exactly one resolve retry.
  - If dirty/manual blocker evidence is found with no active resolve/base-mutating lane, the row returns to manual `merge wait` and scheduler-owned `ResolveWait` membership is cleared.
- Update key hints so `M` reflects intent registration (`resolve pending`) and `resolving` is shown only after scheduler start events.

## Acceptance Criteria

- Pressing `F5` on a `merge wait` row never emits `TuiCommand::ResolveMerge` for the cursor row.
- Pressing `F5` starts/resumes/retries only marked runnable orchestration work, independent of cursor position.
- Pressing `F5` while another change is resolving can still start/resume/retry unrelated runnable work.
- Pressing `M` in Changes view on a `merge wait` row registers scheduler-visible `ResolveMerge` intent and never directly runs merge/resolve outside the scheduler loop.
- Pressing `M` in Changes view on a non-`merge wait` row does not emit `ResolveMerge` and does not display an `M` resolve hint.
- Pressing `M` while another resolve/base-mutating operation is active keeps the target in scheduler-owned `resolve pending`, even if the base/workspace appears dirty due to the active operation.
- Pressing `M` when no resolve/base-mutating operation is active and the workspace/base is dirty results in manual `merge wait` or a transient `resolve pending` that returns to `merge wait`; it must not leave stale `ResolveWait` membership.
- While the orchestrator is running, clean scheduler-owned `ResolveWait` work is promoted one item at a time to `resolving` when no resolve/base-mutating operation is active.
- Worktrees-view `M` continues to merge the selected mergeable worktree branch and remains separate from Changes-view `M`.

## Explicit Completion Conditions

The change is complete when repository evidence shows:

- `src/tui/key_handlers.rs::handle_f5_key` no longer has a cursor-row `merge wait` fast path to `resolve_merge()` and no longer blocks normal orchestration solely because `is_resolving` is true.
- `src/tui/state.rs::resolve_merge`, `src/tui/command_handlers.rs` `TuiCommand::ResolveMerge`, and scheduler code preserve reducer-owned intent semantics for `M`.
- `src/parallel/merge.rs` and/or scheduler classification code explicitly evaluates resolve/base-mutating occupancy before dirty state for merge retry deferral classification.
- Unit/integration tests fail if `F5` triggers `ResolveMerge`, if dirty-during-active-resolve becomes manual `merge wait`, or if clean `ResolveWait` work is not promoted by a running scheduler.
- OpenSpec validation passes with strict mode and implementation-evidence warnings resolved or intentionally justified.

## Out of Scope

- Changing the actual conflict-resolution agent prompt or merge algorithm.
- Introducing durable out-of-worktree state to control resume/resolve routing.
- Changing Worktrees-view `M` merge semantics beyond protecting it from Changes-view behavior conflation.
- Changing the global rule that only one resolve/base-mutating operation may run at a time.
