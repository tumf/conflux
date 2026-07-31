---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/cli/spec.md
  - src/execution/apply.rs
  - src/vcs/git/mod.rs
  - src/vcs/mod.rs
verifications:
  - id: transient-wip-lock-tests
    requirement: Transient Git index-lock contention is retried safely while persistent or unrelated VCS failures remain terminal
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output covering WIP snapshot retry classification, bounded retries, success, exhaustion, and non-retryable errors
    rerun: cargo test transient_wip_commit_lock
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retry transient WIP commit index-lock contention

**Change Type**: implementation

## Problem / Context

Conflux creates a WIP snapshot after each apply iteration. A concurrent or recently terminated Git process can briefly leave the managed worktree's `index.lock` unavailable. Today that command failure immediately fails apply even when the lock disappears moments later and the staged workspace state remains intact. Operators must inspect the repository and retry the whole change manually.

The observed failure occurred in Conflux-owned VCS handling after the agent command completed, not inside the agent command. Recovery therefore belongs in the Conflux WIP snapshot path.

## Proposed Solution

Classify only a `VcsError::Command` from the Git backend as transient when its structured command is the Conflux-owned WIP snapshot `git add -A` or `git commit --no-verify --allow-empty`, stderr reports failure to create an `index.lock` because it already exists, and the reported lock resolves to the current managed worktree's Git directory. Retry the complete `create_progress_commit` snapshot sequence at most three total attempts with a fixed 200 ms delay and no backoff, so both add-stage and commit-stage contention are covered.

Capture `HEAD_before` before each attempt. After an ambiguous command failure, treat the attempt as successful only when HEAD advanced to a commit whose sole parent is `HEAD_before` and whose subject exactly equals the expected WIP message. Otherwise retry only the classified transient contention. Conflux must never delete the lock. Cancellation is checked between attempts at the `create_progress_commit` boundary without adding cancellation state to `WorkspaceManager`; exhausted contention and all unrelated VCS errors remain explicit failures with command, working directory, stderr, and attempt diagnostics.

## Acceptance Criteria

- A WIP snapshot that initially fails because the managed worktree's Git `index.lock` is temporarily unavailable is retried and succeeds when the contention clears within the bounded retry budget.
- A live lock is never deleted or bypassed by Conflux; retries wait and re-attempt normal Git operations.
- Retry handling does not create duplicate WIP commits when the prior attempt actually committed successfully but completion reporting was ambiguous.
- The snapshot receives at most three total attempts separated by a fixed 200 ms delay with no backoff; cancellation observed between attempts suppresses the next attempt, and exhaustion reports the original VCS context plus retry outcome.
- Only structured WIP `git add -A` or WIP commit failures whose reported existing `index.lock` resolves to the current managed worktree are retryable; permission errors, hook failures, identity/configuration errors, merge conflicts, other lock paths, and all other VCS failures remain terminal.
- The workspace remains repository-verifiable: staged or unstaged apply output is preserved when retry succeeds or ultimately fails.

## Explicit Completion Conditions

- `src/execution/apply.rs` owns the three-attempt retry around the complete snapshot sequence; `src/vcs/git/mod.rs` continues to implement the add, staged validation, and commit operations without a general-purpose Git retry.
- Unit tests exercise exact command scope, existing-lock stderr shape, resolved managed-worktree lock identity, three-attempt limit, fixed delay through an injected sleeper, and rejection of near-match failures.
- Integration-style repository tests reproduce an occupied `index.lock`, release it, and observe exactly one WIP commit with the expected message and content.
- Tests cover retry exhaustion, cancellation, ambiguous-success deduplication, and a representative non-retryable Git error.
- `cargo test transient_wip_commit_lock` passes.

## Out of Scope

- Retrying arbitrary Git or VCS failures.
- Deleting `index.lock` files automatically.
- Changing agent-command retry behavior.
- Retrying post-archive merge, upstream push, or remote publication operations.
- Introducing durable workflow state outside the managed workspace.
