---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/changes/archive/2026-05-03-fix-manual-merge-deferred-requeue
  - src/tui/orchestrator.rs
  - src/parallel/merge.rs
  - src/orchestration/state.rs
---

# Stabilize immediate post-archive merge dispatch

**Change Type**: implementation

## Problem / Context

A parallel-mode change can oscillate between `archived` and `merge wait` after archive completion even when there is no preceding active `Resolving` or `Rejecting` change. The observed case was `add-owner-skills-page` in `~/wakumo/avacus/avacuscc-dbot`: it oscillated several times, then eventually reached `merged`.

The user correction is the key premise: when there is no prior resolving/rejecting lane blocker, entering `merge wait` is itself suspicious. `merge wait` should represent a real deferred merge condition or explicit manual retry state, not the default archive-complete state. In the no-blocker case, Conflux should attempt the merge path immediately and move to `merged` or to a justified `MergeDeferred` state only if the merge attempt actually cannot proceed.

This proposal tightens an already-existing canonical requirement, `post-archive-merge-dispatch`, rather than changing merge policy. It does not introduce durable out-of-worktree workflow state and remains compliant with `openspec/CONSTITUTION.md`: workspace and git facts stay authoritative, while UI/log state remains observational.

## Proposed Solution

Make the post-archive path in parallel mode distinguish three cases explicitly:

1. **Active resolving/rejecting blocker exists**: record auto-resumable deferral (`resolve pending`) so scheduler retry can run when the blocker clears.
2. **No active resolving/rejecting blocker exists**: dispatch the immediate merge attempt for the archived workspace instead of settling into `merge wait`.
3. **Immediate merge attempt is actually deferred**: record `merge wait` only when `attempt_merge()` returns a manual non-auto-resumable deferral such as dirty base workspace, incomplete archive verification, or another concrete manual blocker.

The TUI may briefly observe archive completion as an event, but it must not persist `merge wait` merely because archive completed. `merge wait` must be traceable to `MergeDeferred(auto_resumable=false)` or explicit user retry state, not to ordinary no-blocker archive completion.

## Acceptance Criteria

- When a parallel change archives and no other change is actively `Resolving` or `Rejecting`, Conflux attempts the immediate merge path without first waiting for user `M` or normal queue reconciliation.
- In that no-blocker path, reducer/TUI state must not settle on `merge wait` unless a concrete `MergeDeferred(auto_resumable=false)` event is emitted by the merge attempt.
- If another change is actively `Resolving` or `Rejecting`, archive completion may enter `resolve pending` and scheduler-owned retry remains valid.
- Manual merge blockers such as dirty base workspace still enter `merge wait`, clear normal queue intent, and require explicit retry.
- A final `MergeCompleted` or `ResolveCompleted` state remains `merged` and later refreshes do not regress it.
- Regression tests cover the no-blocker archived path so the old `archived <> merge wait` vibration cannot return unnoticed.

## Explicit Completion Conditions

- The no-blocker post-archive path invokes the same immediate merge handling used for archived workspaces, or an equivalent single-change merge path, without requiring a separate user action.
- `src/tui/orchestrator.rs`, `src/parallel/merge.rs`, or equivalent orchestration code no longer leaves a no-blocker archived change in `MergeWait` as a stable state before attempting merge.
- Reducer/TUI tests prove no-blocker archive completion does not produce persistent `merge wait` unless a manual `MergeDeferred(false)` event is processed.
- Existing tests for auto-resumable blocker behavior and manual dirty-base deferral continue to pass.
- Targeted Rust tests pass for post-archive dispatch, merge deferral, reducer state, and TUI display behavior.
- `cflx openspec validate stabilize-parallel-archive-display --strict --evidence warn` passes without unresolved evidence warnings for behavior-changing tasks.

## Out of Scope

- Changing conflict resolution mechanics inside `merge_and_resolve()`.
- Treating dirty base workspace as auto-resumable.
- Auto-stashing, auto-committing, or otherwise mutating a dirty base workspace.
- Adding durable workflow state outside the worktree.
- Reworking unrelated WebUI/server-mode display unless implementation inspection proves it shares this exact post-archive dispatch bug.
