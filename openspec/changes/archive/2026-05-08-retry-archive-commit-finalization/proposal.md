---
change_type: implementation
priority: high
dependencies: []
references:
  - src/execution/archive.rs
  - src/parallel/executor.rs
  - src/agent/runner.rs
  - src/history.rs
  - openspec/specs/parallel-execution/spec.md
---

# Retry Archive Commit Finalization

**Change Type**: implementation

## Problem / Context

Parallel archive execution retries the archive command when the change directory is not moved to `openspec/changes/archive/`, but it does not provide an equivalent bounded repair loop for archive commit finalization after the move succeeds. Once `verify_archive_completion()` observes that the active change directory is gone and the archive entry exists, Conflux exits the archive command retry loop and calls `ensure_archive_commit()`.

`ensure_archive_commit()` currently performs a direct archive commit attempt and, if needed, one AI resolve attempt. If a pre-commit hook, clippy check, formatter, or final verification still prevents a clean `Archive: <change_id>` commit after that single resolve attempt, Conflux returns `Archive commit verification failed` and marks the change errored even when the workspace contains clear, repairable evidence.

This is too eager. Archive finalization failures are often ordinary implementation or hook failures, such as a missing module declaration surfaced by `cargo clippy`. Conflux should give the archive finalization agent bounded feedback-driven retries before terminal error.

## Proposed Solution

Add a bounded archive commit finalization repair loop after archive move verification succeeds:

- Retry direct archive commit and AI archive-finalization resolve attempts until the archive commit is complete or the retry budget is exhausted.
- Feed prior commit/hook stderr, resolve stdout/stderr tail, and archive completion diagnostics into subsequent finalization attempts.
- Treat pre-commit hook modifications as repairable: re-stage and retry rather than terminally failing after one attempt.
- Emit user-visible retry events/logs for archive commit finalization failures so operators can see that Conflux is still repairing the archive commit path.
- Return `Archive commit verification failed` only after the bounded finalization retry budget is exhausted, with the last blocker preserved in the error message.

## Acceptance Criteria

- Archive move success followed by commit hook failure does not immediately become terminal error.
- Archive commit finalization retries are bounded and observable.
- Subsequent finalization attempts receive prior failure context, including hook/clippy/compiler stderr when available.
- If a pre-commit hook modifies files, Conflux re-stages and retries the archive commit path.
- If the finalization agent fixes the blocker and creates a clean `Archive: <change_id>` commit, the archive phase succeeds without re-running the full archive command unnecessarily.
- If the finalization retry budget is exhausted, the terminal error identifies archive commit finalization as the failed phase and includes the last actionable blocker.
- The retry state is runtime-ephemeral and derived from workspace/git state plus in-memory attempt context; it does not introduce durable workflow-control state outside the workspace.

## Explicit Completion Conditions

Complete only when `src/execution/archive.rs` and the parallel archive path in `src/parallel/executor.rs` use a bounded archive commit finalization loop, regression tests prove recoverable commit hook/final verification failures are retried before terminal error, and the final terminal error is emitted only after the configured or constant retry budget is exhausted.

## Out of Scope

- Changing the OpenSpec archive directory structure.
- Skipping or disabling git hooks, clippy, rustfmt, or pre-commit checks.
- Adding out-of-worktree durable workflow-control state.
- Re-running the full archive command when only the archive commit finalization needs repair, unless existing file-state verification proves the archive move regressed.
