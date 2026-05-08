# Design: Resume Archived Dirty Workspaces

## Why this is separate from archive finalization retry

`retry-archive-commit-finalization` addresses the intra-run finalization loop: once archive move succeeds, Conflux should keep trying to finish the `Archive: <change_id>` commit before giving up.

This proposal handles the next layer. If a run still exits with archive finalization failure, Conflux must not strand the workspace in a recoverable archived-dirty state that the scheduler never reclaims.

## Repository-visible recovery signal

An archived dirty workspace is observable without external durable state:

- `openspec/changes/<change_id>/` is absent in the workspace
- `openspec/changes/archive/<date>-<change_id>/` exists
- `is_archive_commit_complete(...)` is false
- `has_archive_files(...)` is true
- git working tree is dirty or latest commit subject is still pre-finalization

That signal is exactly the kind of workspace-local state the constitution allows to drive behavior.

## Target behavior

When the scheduler or resumed runtime sees this state, it should treat the change as recoverable archive-finalization work, not as dead history.

Concretely:

1. Detect archived dirty state from repository-visible evidence.
2. Recreate scheduler ownership / retry intent for that change.
3. Dispatch only archive-finalization repair unless archive-move state regressed.
4. Preserve bounded retry semantics and terminal failure only after exhaustion.

## Lifecycle implications

This likely needs either:

- a dedicated scheduler-visible wait/retry membership for archived dirty workspaces, or
- reuse of an existing reducer-owned retry state with explicit semantics for archive-finalization recovery.

What matters is that the state is not terminal merely because one run ended.

## User-visible taxonomy

The system should distinguish at least three cases:

- archive move not complete
- archive move complete, commit incomplete but recoverable
- archive finalization exhausted, terminal error

This helps operators understand whether the system is still repairing or truly stuck.
