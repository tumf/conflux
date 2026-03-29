# Change: Fix worktree branch cleanup and stale-branch recovery

**Change Type**: implementation

## Problem/Context

Parallel Git worktree execution can fail when a stale worktree path is cleaned up but the same-named local branch still exists. The current recovery path classifies the first failure as a stale-path case, retries after prune, and can then fail again with an existing-branch error instead of attaching the remaining branch when it is safe to do so.

Worktree deletion behavior is also expected to remove the associated local branch when deleting from the TUI Worktrees view via `D`, while treating branch deletion as best-effort and non-fatal.

## Proposed Solution

- Update Git worktree creation recovery so that a stale-path retry may fall through into the existing-branch attach flow when the branch remains and is not checked out elsewhere.
- Make the expected branch-deletion behavior explicit for TUI `D` deletion and align implementation verification around shared worktree deletion flows.
- Preserve safety constraints: never auto-attach if the branch is already checked out in another worktree, and never fail the overall deletion solely because branch deletion fails.

## Acceptance Criteria

- Creating a worktree succeeds when the initial `git worktree add -b <branch>` fails due to a stale path, pruning removes the stale registration/path, and the retry discovers that the branch already exists but is not checked out elsewhere.
- Creating a worktree still fails with a clear classified error when the branch is already checked out in another worktree.
- Deleting a worktree from the TUI Worktrees view with `D` attempts to delete the associated local branch after worktree removal.
- If branch deletion fails or the branch is already absent, worktree deletion remains successful and a warning is logged.
- Verification covers the VCS recovery path and the TUI delete flow with repository tests or targeted command-handler tests.

## Out of Scope

- Changing branch naming rules or worktree directory layout.
- Automatically deleting branches that are still checked out by another worktree.
- Changing unrelated server/web worktree lifecycle semantics beyond shared branch-cleanup expectations.
