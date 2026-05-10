# Design: Stale merge retry worktree status

## Current failure path

1. A change archives successfully in a parallel workspace.
2. Merge retry uses the archived workspace path as an archive-completion verification root.
3. Cleanup or worktree reconciliation removes that path before a retry uses it.
4. `is_archive_commit_complete` treats the supplied path as a repository root and runs `git status --porcelain` in that directory.
5. Missing directory produces `No such file or directory`, which is wrapped as a manual `MergeDeferred` reason and can be logged repeatedly.

## Design principles

- Archive-completion verification for merge readiness should use repository-visible file/git evidence from an existing repository root.
- Worktree paths are valid inputs for workspace cleanup, acceptance-state cleanup, and workspace-local resume inspection, but they are unsafe as the only root for base merge readiness once cleanup can race with retry scheduling.
- Missing retry worktrees should converge. They are stale scheduler evidence, not a new user action requirement.
- UI deduplication is a guardrail, not the root fix. The scheduler must still avoid retrying stale evidence forever.

## Implementation shape

- In merge-attempt code, pass `self.repo_root` or another validated existing root to archive completion checks that need `git status`.
- Keep workspace paths available for cleanup-specific operations under names that do not imply they are archive directories.
- In retry dispatch, check `WorkspaceInfo.path.exists()` and base integration before calling merge verification. If the path is gone and the change is already merged, mark retry completed. If the path is gone and no repository-visible archived/merged evidence remains, clear stale retry intent with a single diagnostic.
- In TUI error handling, store the last merge-deferred diagnostic signature per change or otherwise compare adjacent repeated diagnostics. Suppress only exact repeats; changed reasons must still be visible.

## Trade-offs

- Using the stable repo root for archive verification may miss workspace-local dirty state, but merge readiness after archive should be governed by base/repository-visible archive evidence and valid workspace existence checks, not by running git in a path that may already be cleaned up.
- TUI dedupe protects the operator from log floods even if a future retry loop regresses, but it must not become workflow-control state.

## Compatibility

This change preserves the existing `ResolveWait` and `MergeWait` model. It does not introduce external durable workflow state and remains compatible with the constitution's workspace-local evidence rule.
