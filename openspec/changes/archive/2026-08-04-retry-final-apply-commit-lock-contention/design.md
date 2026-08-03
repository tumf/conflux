# Design: transient final Apply commit lock retry

## Scope

The policy applies only to the hook-enabled final Apply commit boundary. It complements the archived WIP snapshot retry without broadening retry behavior to arbitrary Git commands.

## Decisions

### Classify from structured command data

Retry requires all of the following:

- a Git `VcsError::Command` from final Apply finalization
- a structured command matching final `git add -A`, add-and-commit, or amend
- stderr stating that Git could not create an existing `index.lock`
- a reported path resolving to the current managed worktree Git directory

Hook rejection and every near match remain outside this policy.

### Retry complete finalization preparation

Each attempt re-reads whether the worktree needs add-and-commit or amend, stages and validates when dirty, and runs the normal verified commit. This avoids replaying a stale mode decision after another actor changes repository state.

### Preserve verification hooks

Every commit attempt uses the existing verified commit arguments. `--no-verify` remains exclusive to WIP snapshots. A hook exit follows `RepositoryRejected`; it never enters lock retry.

### Prove ambiguous success

Before each attempt, capture HEAD and the expected workspace tree. If command reporting is ambiguous, accept success only when current HEAD is the exact expected successor, has subject `Apply: <change-id>`, and contains the expected tree. Otherwise apply the narrow lock classifier or return the terminal error.

### Bound waiting

Use three total attempts and fixed 200 ms delays. Cancellation is checked before sleeping and before retry. Conflux never unlinks a lock because ownership cannot be inferred safely.

## Constitution Alignment

Recovery uses only current process state and repository evidence. It adds no durable external workflow state, and final Apply completion is recognized only from verified Git evidence.

## Split Rationale

This proposal is independent of `wait-for-apply-process-group-before-git-finalization`. Process cleanup removes a known internal contender; this retry handles transient contention that can still originate outside the owned process group. Neither implementation consumes repository output from the other, so no hard dependency is declared.
