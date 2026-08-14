## MODIFIED Requirements

### Requirement: Serial Apply Iteration WIP Commits

Managed-worktree apply execution MUST preserve each iteration's latest work as a new WIP snapshot commit after successful, failed, or no-progress apply results. The message MUST use `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})`; the snapshot MUST be equivalent to `git add -A` followed by `git commit --no-verify --allow-empty`; and it MUST NOT amend an existing WIP commit.

Only when Conflux-owned WIP `git add -A` or commit cannot create an `index.lock` because the current managed worktree Git directory already contains that lock, Conflux MUST retry the complete `create_progress_commit` snapshot sequence up to five total attempts at fixed 500 millisecond intervals without backoff. This retry policy MUST NOT delete or bypass the lock and MUST NOT apply to general Git commands. The only permitted deletion is the same-dispatch post-quiescence reclamation defined by the `process-execution` capability, completed before this retry policy begins.

Before each attempt Conflux MUST record `HEAD_before`. A failed attempt counts as committed only when HEAD advanced to a commit whose sole parent is `HEAD_before` and whose subject exactly matches the expected WIP message. A same-subject commit elsewhere in history MUST NOT prove success. Runtime MUST check cancellation after a retryable failure, before waiting, and before each next attempt. Exhausted lock contention and non-lock VCS errors MUST preserve workspace content and return actionable diagnostics.

#### Scenario: Managed-worktree iterations always snapshot current work

- **GIVEN** a managed-worktree apply iteration finishes successfully, fails, or makes no task progress
- **WHEN** iteration finalization runs
- **THEN** current staged and unstaged work is saved in exactly one new WIP snapshot commit
- **AND** no existing WIP commit is amended

#### Scenario: Transient index lock clears within retry budget

- **GIVEN** a Conflux-owned WIP snapshot reports an existing `index.lock` in the current managed worktree Git directory
- **AND** the lock becomes available before the fifth total attempt
- **WHEN** Conflux retries the complete snapshot sequence after each fixed 500 millisecond delay
- **THEN** the expected WIP commit is created exactly once
- **AND** the retry policy does not delete or bypass the lock

#### Scenario: Ambiguous commit completion does not duplicate WIP

- **GIVEN** a WIP attempt captured `HEAD_before` and then reported failure
- **AND** current HEAD advanced to a commit whose sole parent is `HEAD_before` and whose subject exactly matches the expected WIP message
- **WHEN** Conflux evaluates another attempt
- **THEN** it recognizes the prior attempt as committed
- **AND** it does not create a duplicate WIP commit

#### Scenario: Persistent lock and cancellation preserve workspace state

- **GIVEN** lock contention exhausts five attempts or cancellation is observed before another attempt
- **WHEN** progress-commit orchestration stops
- **THEN** no further attempt starts
- **AND** workspace content and the lock file remain untouched by the retry policy
- **AND** diagnostics identify the command, working directory, contention, and attempts

#### Scenario: Non-lock VCS failure is not retried

- **GIVEN** a WIP snapshot fails because of a permission, identity, configuration, hook, conflict, or other non-lock VCS error
- **WHEN** Conflux classifies the failure
- **THEN** it returns the structured VCS failure without transient-lock retry

<!-- Expected canonical result after archive: WIP lock contention will use five total attempts at fixed 500 millisecond intervals, while lock deletion remains outside retry and limited to the process-execution post-quiescence boundary. -->
