---
change_type: implementation
priority: high
references:
  - src/execution/state.rs
  - src/parallel/dispatch.rs
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - src/tui/worktrees.rs
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Resume Archived Dirty Workspaces

**Change Type**: implementation

## Problem / Context

A parallel change can successfully move into `openspec/changes/archive/` yet still fail archive commit finalization, leaving the workspace in an `Archiving (files moved, commit incomplete)` state. Today, Conflux can emit a terminal `Archive failed` error for that condition and then stop scheduling meaningful repair work even though the workspace still contains clear repository-visible recovery evidence: archive files exist, the active change directory is gone, and the remaining blocker is only archive commit completion.

This leaves an archived-dirty workspace stranded. Subsequent scheduler cycles can observe the `Archiving` state but do not automatically re-own the workspace as a recoverable repair target. The system idles instead of resuming archive finalization.

## Proposed Solution

Introduce scheduler-owned recovery semantics for archived-but-commit-incomplete workspaces:

- Treat `Archiving (files moved, commit incomplete)` as a recoverable runtime state rather than an immediately terminal archive failure.
- Reconstruct retry intent for archived dirty workspaces on subsequent scheduler cycles using repository-visible workspace state rather than external durable state.
- Route archived dirty workspaces back into archive finalization repair without re-running the full archive command unnecessarily.
- Distinguish archive command failure from archive finalization recovery in lifecycle state, events, and logs.
- Permit terminal error only when archive finalization retry/exhaustion policy concludes the workspace is no longer recoverable in the current run.

## Acceptance Criteria

- A workspace with archive files present, active change directory absent, and incomplete archive commit is treated as recoverable scheduler-owned work.
- After a terminal-seeming archive finalization failure, the next scheduler cycle can rediscover the archived dirty workspace from repository-visible state and resume repair work.
- Archived dirty recovery does not require out-of-worktree durable state.
- Recovery of archived dirty workspaces does not re-run the full archive command unless file-state verification shows the archive move regressed.
- User-visible lifecycle/events distinguish `archive move incomplete`, `archive commit incomplete but recoverable`, and truly terminal archive failure.
- A workspace is not left permanently idle in `Archiving` when bounded recovery work is still available.
- Truly exhausted archive finalization failures still surface a terminal error with the final blocker.

## Explicit Completion Conditions

Complete only when runtime state derivation, scheduler queue ownership, and archive recovery dispatch paths can re-own an archived dirty workspace after archive finalization failure using repository-visible evidence alone, with regression tests proving the scheduler resumes repair instead of idling, and terminal error is emitted only after the bounded recovery policy is exhausted.

## Out of Scope

- Changing the on-disk archive layout.
- Disabling hooks or reducing validation rigor during archive commit creation.
- Introducing durable external resume state outside the workspace/git/base-tree contract.
- Rewriting the archive command itself when only post-move archive commit finalization is incomplete.
