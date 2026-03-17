# Change Proposal: fix-parallel-resume-state-detection

## Problem / Context

- `cflx run --parallel` and parallel worktree dispatch can automatically reuse existing workspaces unless `--no-resume` is set.
- Resume behavior currently depends on workspace state detection plus task-progress checks that can read `tasks.md` from archived locations inside the workspace.
- In the observed case, an existing worktree for `add-create-start-lifecycle` was treated as already complete and immediately moved toward a final apply commit, even though the user's intent was to understand why the change was not being freshly processed.
- This makes resume behavior hard to distinguish from a fresh start and can produce surprising "already complete" behavior when a stale or previously archived workspace is reused.
- The issue is adjacent to, but distinct from, start-time rejection of uncommitted changes. It needs its own proposal because it concerns workspace reuse semantics and resume-state observability.

## Proposed Solution

- Clarify and tighten the rules for when an existing parallel workspace may be automatically resumed versus when it must be recreated or surfaced as a resumable state to the user.
- Make the user-visible reporting for reused workspaces explicit so CLI users can tell when execution resumed from an existing workspace state instead of starting fresh.
- Ensure "already complete" detection based on archived `tasks.md` in a reused workspace only happens in states where that interpretation is intended.
- Add regression coverage for workspace reuse paths that currently jump directly to complete/archive handling.

## Acceptance Criteria

- When `cflx run --parallel` reuses an existing workspace, the CLI clearly reports that the change is resuming from detected workspace state rather than starting fresh.
- A reused workspace is not treated as freshly startable if state detection says it is already archived, merged, or otherwise beyond apply unless that behavior is explicitly intended by the resume rules.
- "Already complete" detection from archived `tasks.md` does not silently mask the fact that execution came from a reused workspace state.
- Regression tests cover reused workspaces whose archived task files currently cause surprising immediate completion behavior.

## Out of Scope

- Start-time filtering of uncommitted changes before dispatch; that is covered by `fix-parallel-start-rejection-state`.
- Redesigning the entire workspace lifecycle model.
