---
change_type: implementation
priority: high
dependencies:
  - fix-post-merge-deferred-false-warning
references:
  - openspec/CONSTITUTION.md
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/execution/archive.rs
  - src/tui/state/event_handlers/errors.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/tui-error-handling/spec.md
---

# Fix stale merge retry worktree status

**Change Type**: implementation

## Problem/Context

A deferred parallel merge can repeatedly log this operator-visible warning after an archived change's worktree has already been removed or is otherwise stale:

- `Merge deferred for <change>: Failed to verify archive completion for '<change>': Git command failed: Failed to check git status: No such file or directory (os error 2)`

The inspected path shows `src/parallel/merge.rs` passes a worktree path into `is_archive_commit_complete`, while `src/execution/archive.rs` treats that path as a repository root and runs `git status --porcelain` there. If cleanup removed the worktree between retry scheduling and archive-completion verification, `Command::current_dir` fails before merge eligibility can be evaluated.

This can repeat because reducer-owned `ResolveWait` retry dispatch may reattempt the same stale merge retry on subsequent lane-release or scheduler ticks. The final merge may still succeed later, but the TUI presents a burst of scary warnings for a transient stale path.

The fix must obey `openspec/CONSTITUTION.md`: workflow-control decisions must remain derivable from repository/workspace git state and must not depend on hidden durable logs or UI state.

## Proposed Solution

Make deferred merge retry robust to stale worktree paths:

- Use a stable, existing repository root for archive-completion verification when deciding whether an archived change may be merged.
- Treat deleted or inconsistent retry worktree paths as stale retry evidence before invoking `git status` in that missing directory.
- Clear or suppress scheduler retry intent when repository-visible evidence proves the change is already merged, already cleaned up, or no longer has a valid retry worktree.
- Bound repeated TUI diagnostics for identical `MergeDeferred` reasons so one stale path does not flood the log while the scheduler converges.

This proposal is narrower than `fix-post-merge-deferred-false-warning`: that proposal addresses duplicate post-merge false deferrals after base integration; this proposal addresses stale/missing retry worktree paths and repeated `No such file or directory` diagnostics.

## Acceptance Criteria

- Deferred merge archive-completion verification MUST NOT run `git status` with `cwd` set to a missing worktree path.
- A deleted or stale worktree path discovered during `ResolveWait` merge retry MUST be handled as stale retry evidence, not as a repeated manual merge blocker.
- If repository-visible base evidence shows the change is already merged or no retry worktree exists, scheduler retry intent for that change MUST converge instead of repeatedly emitting the same warning.
- Legitimate manual merge deferrals, such as dirty base or unresolved merge conflicts in an existing repository root, MUST remain visible and actionable.
- TUI logs MUST NOT append unbounded identical `MergeDeferred` warnings for the same change, reason, and retry state.

## Explicit Completion Conditions

- `src/parallel/merge.rs` archive verification uses an existing repo root for `is_archive_commit_complete` or otherwise validates the path before any `git status` call.
- `src/parallel/queue_state.rs` stale retry handling avoids invoking merge verification against missing worktrees and clears/suppresses retry intent when repository-visible evidence indicates no retry is possible or necessary.
- `src/tui/state/event_handlers/errors.rs` or the upstream event path deduplicates/rate-limits identical merge-deferred warning logs without hiding distinct reasons.
- Regression tests cover deleted worktree retry, valid dirty-base manual deferral, and duplicate TUI warning suppression.
- Focused Rust tests and OpenSpec validation pass.

## Out of Scope

- Replacing the reducer-owned `ResolveWait` / `MergeWait` model.
- Introducing durable retry state outside repository/workspace evidence.
- Changing acceptance/archive semantics unrelated to post-archive merge retry.
- Solving all duplicate post-merge race cases already covered by `fix-post-merge-deferred-false-warning`.
