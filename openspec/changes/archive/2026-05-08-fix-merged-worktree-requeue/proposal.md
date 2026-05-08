---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/parallel/queue_state.rs:1215
  - src/parallel/dispatch.rs:394
  - src/parallel/dispatch.rs:677
  - src/parallel/tests/executor.rs:3679
---

# Change: Fix merged worktree requeue after archive completion

**Change Type**: implementation

## Problem / Context

A previously processed change can remain as a Git worktree after the change has already been archived and merged into the base branch. In the observed run, `fix-dependency-target-handling` reached `state=Merged -> Terminal`, then scheduler queue reconciliation immediately rediscovered the same leftover worktree as an `archived_dirty_repair_candidate` and added it back to the scheduler-local queue.

That requeue is wrong. Once repository-visible evidence shows the change is already merged, the leftover worktree is terminal cleanup residue, not resumable apply/archive work. Requeueing it can cause a fresh apply path and acceptance to run again for a change that is already archived and merged.

The fix must preserve the legitimate archived-dirty repair behavior for interrupted archive finalization, while preventing merged terminal workspaces from becoming repair candidates.

## Proposed Solution

Update scheduler queue reconciliation and archived-dirty candidate discovery so a worktree is considered repairable only when workspace-local evidence shows it is not already merged into the base branch.

The implementation should use existing workspace-local workflow inputs only:

- workspace file state
- workspace Git state
- base-branch tree comparison

The implementation must not introduce durable external workflow state. This follows `openspec/CONSTITUTION.md` and keeps resume routing derivable from the workspace alone.

## Acceptance Criteria

- A leftover worktree for a change already merged into the base branch is not added to scheduler-local queued work by queue reconciliation.
- A leftover worktree that is archived but not yet merged continues to enter archive-complete merge handoff or merge wait handling.
- An archived-dirty workspace whose archive move is complete but commit finalization is incomplete remains repairable when it is not merged.
- Reconciliation emits no repeated user-visible archived-dirty repair diagnostics for terminal merged worktrees.
- The original observed failure mode is covered by a regression test: after a change is detected as `Merged`, the same archived worktree is not rediscovered as an `archived_dirty_repair_candidate` and cannot re-enter apply/acceptance.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` or the archived-dirty candidate helper gates repair candidate insertion on a workspace-local merged-state check before adding the candidate to `queued`.
- `src/parallel/dispatch.rs` keeps `WorkspaceState::Merged` terminal and does not hand it to apply, acceptance, archive, or archived-dirty repair handoff.
- `src/parallel/tests/executor.rs` includes a regression test for merged leftover worktree reconciliation and a preservation test for legitimate archived-dirty repair.
- Targeted tests for the new reconciliation behavior pass.
- Default Rust verification and lint/typecheck commands are run, or any blocker is recorded with the exact command and failure output.

## Out of Scope

- Changing acceptance verdict parsing or acceptance prompt behavior.
- Changing the archive command itself.
- Adding out-of-worktree durable resume state.
- Broad worktree cleanup policy redesign beyond preventing terminal merged worktrees from being requeued.
