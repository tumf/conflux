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

# Normalize post-archive status transitions

**Change Type**: implementation

## Problem / Context

Post-archive status management in parallel mode is inconsistent. After a change archives, the UI/reducer/scheduler can temporarily or repeatedly show states that do not match the actual merge lane condition.

Observed symptoms from `add-owner-skills-page` in `~/wakumo/avacus/avacuscc-dbot`:

- `merged <> archived` vibrates for a while, then eventually settles on `merged`, even though there is no visible `resolving` phase.
- `merge wait` appears even though there is no preceding active resolving/merge blocker, then eventually settles on `merged`.

Expected post-archive state rules are:

- If another merge/resolve lane is already in progress, the archived change should become `resolve pending`.
- If merge cannot proceed because the relevant workspace/base state is dirty or otherwise manually blocked, the archived change should become `merge wait` with a concrete deferral reason.
- Otherwise, the archived change should transition into `resolving` while merge handling runs, then become `merged` when merge completes.

The current behavior makes `archived` and `merge wait` look like ordinary intermediate states even when they are not justified by repository/workspace facts. That violates truthful lifecycle display: users cannot tell whether Conflux is waiting for another merge, waiting for manual cleanup, actively merging, or already done.

This proposal tightens the existing `post-archive-merge-dispatch` requirement and adds explicit no-vibration status invariants. It does not introduce durable out-of-worktree workflow state and remains compliant with `openspec/CONSTITUTION.md`: workspace and git facts stay authoritative, while UI/log state remains observational.

## Proposed Solution

Define and implement a single reducer-owned post-archive state decision table for parallel mode:

1. **Merge/resolve lane occupied**: set `ResolveWait` / `resolve pending`. This covers another active merge/resolve operation that prevents immediate merge handling for the newly archived change.
2. **Manual merge blocker exists**: set `MergeWait` / `merge wait` only after merge handling detects a concrete non-auto-resumable blocker such as dirty base workspace, dirty archive workspace, incomplete archive verification, or missing archive evidence.
3. **No blocker exists**: transition to `Resolving` / `resolving`, run merge handling immediately, then transition to `Merged` / `merged` on completion.

The implementation should make `archived` terminal only for serial mode. In parallel mode, `archived` is a repository milestone, not a stable visible lifecycle state after post-archive routing begins.

## Acceptance Criteria

- After parallel archive completion, exactly one of the expected paths is selected from current reducer/workspace/git facts: `resolve pending`, `merge wait`, or `resolving -> merged`.
- `resolve pending` is used only when another merge/resolve lane is in progress and the archived change is eligible for automatic retry.
- `merge wait` is used only when merge handling has attempted or verified enough context to find a concrete manual blocker.
- In the normal no-blocker path, the visible state transitions to `resolving` while merge handling runs and then to `merged` when merge completes.
- `merged` is terminal for display purposes: later `ChangeArchived`, `ChangesRefreshed`, workspace observations, or cleanup events must not regress it to `archived`, `merge wait`, or `resolve pending`.
- `archived <> merged` and `merge wait -> merged` vibration without a preceding justified `resolving`, `resolve pending`, or manual deferral is forbidden by regression tests.
- Serial mode still displays `archived` for archive-terminal changes.

## Explicit Completion Conditions

- `src/orchestration/state.rs` or equivalent reducer state code encodes the post-archive decision table without treating parallel `archived` as a stable terminal display.
- `src/tui/orchestrator.rs`, `src/parallel/merge.rs`, or equivalent orchestration code emits/apply events so the no-blocker path visibly enters `resolving` before `merged`.
- Manual deferral paths still emit `MergeDeferred(auto_resumable=false)` with enough reason text to explain `merge wait`.
- Auto-resumable blocked paths still emit a state that explains `resolve pending`.
- Tests cover all three post-archive paths and both reported vibration regressions.
- Targeted Rust tests pass for post-archive dispatch, merge deferral, reducer state, and TUI display behavior.
- `cflx openspec validate stabilize-parallel-archive-display --strict --evidence warn` passes. Any remaining evidence warning must be explicitly justified in implementation notes.

## Out of Scope

- Changing conflict resolution mechanics inside `merge_and_resolve()` beyond lifecycle event/status emission needed for truthful display.
- Treating dirty base workspace as auto-resumable.
- Auto-stashing, auto-committing, or otherwise mutating a dirty base workspace.
- Adding durable workflow state outside the worktree.
- Reworking unrelated WebUI/server-mode display unless implementation inspection proves it shares this exact post-archive state bug.
