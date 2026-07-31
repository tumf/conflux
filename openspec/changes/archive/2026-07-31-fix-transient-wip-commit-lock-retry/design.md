# Design: transient WIP commit lock retry

## Scope

The retry belongs at the Conflux WIP snapshot boundary. The agent command has already returned, and the failed side effect is the orchestrator-owned Git snapshot. Agent retry policy and general VCS retry behavior remain unchanged.

## Decisions

### Classify narrowly

A failure is transient only when all conditions hold:

- the value is `VcsError::Command` with the Git backend
- the structured command is the Conflux-owned iteration-snapshot `git add -A` or `git commit --no-verify --allow-empty`
- stderr reports that Git was unable to create an `index.lock` because the file exists
- the reported lock path resolves to the Git directory of the current managed worktree

Near matches, including another repository's lock, another Git command, generic lock prose, permission failures, and malformed or unresolved lock paths, are terminal.

### Retry the complete snapshot

The retry loop lives at `create_progress_commit`, above the `WorkspaceManager` boundary. Each attempt repeats `snapshot_working_copy` and `create_iteration_snapshot`; for Git, the former is a no-op and the latter runs `git add -A`, staged-snapshot validation, and the WIP commit. This covers contention during either add or commit without introducing a general Git-command retry.

### Never remove the lock

Conflux waits and retries normal Git operations. It does not unlink `index.lock`, because lock ownership cannot safely be inferred from file age alone and another live process may be mutating the index.

### Prove idempotency from repository state

Capture `HEAD_before` before every attempt. If a command reports failure, recognize ambiguous commit success only when current HEAD differs from `HEAD_before`, current HEAD has exactly one parent equal to `HEAD_before`, and its subject exactly matches the expected WIP message. A subject found elsewhere in history is not success evidence.

### Bound retries and honor cancellation

Allow three total attempts, with a fixed 200 ms delay and no backoff. Keep sleeping injectable in tests so exhaustion and cancellation tests complete without wall-clock delay. Cancellation is checked after a retryable failure and before sleeping, then again before starting the next attempt. The token remains above `WorkspaceManager`; the VCS trait and Git backend do not gain cancellation plumbing. Exhaustion returns the original structured VCS failure augmented with attempt context.

## Constitution Alignment

The design derives recovery decisions from workspace files and Git state. It introduces no out-of-worktree durable workflow state and does not infer implementation or acceptance completion from narrative output.

## Verification Strategy

Unit tests cover classification and retry-state decisions. Temporary-repository tests hold a real `index.lock` to exercise Git behavior, then verify commit count, subject, tree content, retained workspace state on failure, cancellation, and non-retryable errors.
