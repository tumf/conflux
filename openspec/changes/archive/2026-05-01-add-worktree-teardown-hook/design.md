# Design: Worktree teardown hook

## Overview

The implementation should make teardown-aware deletion the default path for Conflux-managed Git worktrees. Existing low-level Git removal can remain available for narrowly-scoped internal use, but user-facing and orchestration cleanup paths should not bypass teardown accidentally.

## Hook location and execution

Conflux should inspect the worktree being deleted, not the base repository, for:

```text
<worktree-root>/.wt/teardown
```

When the file exists and is executable, Conflux runs it before Git worktree removal:

- command: `<worktree-root>/.wt/teardown`
- cwd: `<worktree-root>`
- env: `ROOT_WORKTREE_PATH=<base-repository-root>`
- stdin: null / closed

This lets `.wt/teardown` read worktree-local state such as `.wt/state.env` without relying on global state.

## Failure policy

Default deletion must be safe-by-default:

1. If teardown succeeds, proceed with `git worktree remove`.
2. If teardown is missing, proceed as before.
3. If teardown exists but is not executable, warn and proceed without running it.
4. If teardown fails, abort before `git worktree remove` and return/report diagnostics.
5. If the caller explicitly sets `skip_teardown`, do not block deletion on teardown. The implementation may either skip execution entirely or treat failure as warning, but the behavior must be explicit in logs and user-facing messages.

`skip_teardown` is intentionally separate from Git's existing `--force`; it means “do not let teardown block cleanup”, not “force Git to delete”.

## Shared API shape

A possible internal shape:

```rust
pub struct WorktreeRemoveOptions {
    pub skip_teardown: bool,
}

pub enum WorktreeTeardownOutcome {
    Missing,
    NotExecutable,
    Skipped,
    Succeeded { stdout: String, stderr: String },
}

pub async fn run_worktree_teardown(repo_root: &Path, worktree_path: &Path) -> VcsResult<WorktreeTeardownOutcome>;

pub async fn worktree_remove_with_teardown(
    repo_root: &Path,
    worktree_path: &Path,
    options: WorktreeRemoveOptions,
) -> VcsResult<WorktreeTeardownOutcome>;
```

The exact names are flexible, but the implementation should centralize behavior so deletion paths cannot drift.

## Deletion paths to review

At minimum, implementation should review and wire these areas:

- `src/vcs/git/mod.rs`: tracked workspace cleanup, stale dependency-resolved cleanup, inconsistent worktree cleanup
- `src/orchestration/rejection.rs`: rejected worktree cleanup
- `src/tui/command_handlers.rs`: manual worktree deletion
- `src/server/api/worktrees.rs`: server mode worktree deletion
- `src/web/api.rs`: legacy web worktree deletion
- `src/server/proposal_session.rs`: proposal session close and merge cleanup
- dashboard API/UI files if the skip option is exposed in WebUI

## Constitution compatibility

`.wt/teardown` and `.wt/state.env` must not become authoritative workflow-control inputs. They are cleanup mechanisms for external side effects. Resume routing, acceptance routing, archive routing, and next-action decisions must continue to derive from workspace-local file/git state and base-branch tree comparison.

## Verification strategy

Use temporary repositories and executable shell scripts for local tests. Avoid real Docker/Postgres/Redis dependencies. Tests can simulate external cleanup by writing marker files from `.wt/teardown` and checking that marker creation, cwd, env, and failure behavior match the spec.
