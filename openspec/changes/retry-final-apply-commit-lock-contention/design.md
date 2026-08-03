# Design: transient final Apply commit lock retry

## Scope and Dependency

The policy applies only to hook-enabled final Apply commit after `wait-for-apply-process-group-before-git-finalization` has produced confirmed quiescence. It consumes that change's typed cleanup result and Apply gate; retry without it could hide a live Apply descendant, so the dependency is hard rather than roadmap ordering.

## Immutable Finalization Plan

Create one plan before mutation:

- baseline HEAD OID and its parent OIDs
- fixed mode: `AddAndCommit` or `Amend`
- exact subject `Apply: <change-id>`
- expected tree OID

Initial mode detection uses `git --no-optional-locks status --porcelain`. For `AddAndCommit`, create an ephemeral isolated index from the current index, run `git add -A` against that index, and use `git write-tree` as the complete intended snapshot. This includes staged, unstaged, deleted, and untracked content without touching the real index. For `Amend`, require clean status and use baseline HEAD's tree.

## Retry Preflight

Before every attempt after the first:

1. Check mode-specific exact-success evidence.
2. If not successful, require current HEAD equals baseline HEAD.
3. Recompute the complete workspace tree with an isolated index and require it equals expected tree.
4. Require the fixed mode's real-index state is compatible; never re-derive or switch mode.
5. For add-and-commit, run real `git add -A` only after checks, then require real `git write-tree` equals expected tree.
6. Run the fixed verified commit command.

Any mismatch is a terminal concurrent-mutation error. Conflux does not absorb, reset, or reconcile external changes.

## Mode-Specific Success Proof

| Mode | Exact success evidence |
| --- | --- |
| AddAndCommit | HEAD differs from baseline, has exactly baseline HEAD as sole parent, exact subject, expected tree |
| Amend | HEAD differs from baseline, has exactly the same parent set/order as baseline HEAD, exact subject, expected tree |

A same-subject historical commit, external HEAD advance, or matching tree with wrong lineage is not success.

## Retry Eligibility and Hooks

Eligibility requires a structured finalization command, non-exit-1 terminal status, exact fatal existing-`index.lock` stderr, and lock identity resolving to the current managed worktree. Exit code 1 stays `RepositoryRejected`.

Final commits always run hooks. Automatic retry is permitted only for the top-level Git lock-acquisition failure before hooks execute. Temporary-repository tests install a counting pre-commit hook and hold a real lock; failed eligible attempts must leave the counter at zero and eventual success must make it exactly one. If platform behavior cannot prove this invariant, commit-command lock failure is terminal on that platform rather than retried.

## Bounded Waiting

Use three total attempts and fixed 200 millisecond delays through an injected sleeper. Check cancellation before sleeping and before retry. Never unlink a lock.

## Constitution Alignment

The plan and retry state are process-local. Success and drift decisions use workspace and Git evidence. External mutation is surfaced rather than silently committed.
