## ADDED Requirements

### Requirement: Final Apply commit retries narrowly classified index-lock contention

When final Apply finalization fails because a structured finalization Git command cannot create the current managed worktree's existing `index.lock`, Conflux MUST retry the complete finalization sequence at most three total attempts with a fixed 200 millisecond delay and no backoff. Conflux MUST preserve repository verification hooks, MUST NOT delete or bypass the lock, and MUST NOT apply this policy to hook rejection or unrelated VCS failures. Completion MUST be proven from repository state so ambiguous command reporting cannot create duplicate final commits.

#### Scenario: transient amend lock clears

- **GIVEN** a WIP snapshot leaves the Apply worktree clean
- **AND** final `git commit --amend --allow-empty` reports the current managed worktree's existing `index.lock`
- **WHEN** the lock clears within the bounded retry budget
- **THEN** Conflux retries normal hook-enabled finalization
- **AND** exactly one final `Apply: <change-id>` commit exists
- **AND** Acceptance may start only after repository state proves that commit succeeded

#### Scenario: transient add-and-commit lock clears

- **GIVEN** final Apply finalization must stage dirty workspace content
- **AND** finalization `git add -A` or the subsequent verified commit reports the current managed worktree's existing `index.lock`
- **WHEN** the lock clears within three total attempts
- **THEN** Conflux repeats complete finalization preparation from current repository state
- **AND** the final hook-enabled commit contains the expected workspace tree exactly once

#### Scenario: persistent lock exhausts bounded retries

- **GIVEN** eligible managed-worktree `index.lock` contention persists for all three attempts
- **WHEN** the retry budget is exhausted
- **THEN** final Apply fails with structured command, workspace, lock, stderr, and attempt diagnostics
- **AND** Conflux does not delete the lock
- **AND** workspace contents remain available for explicit recovery

#### Scenario: repository hook rejection is not lock contention

- **GIVEN** a hook-enabled final Apply commit runs
- **WHEN** a repository hook rejects that commit
- **THEN** Conflux does not consume the index-lock retry budget
- **AND** it routes the rejection through the existing bounded Apply commit-repair behavior

#### Scenario: unrelated VCS failures remain terminal

- **GIVEN** finalization fails for another worktree's lock, malformed lock output, permission or configuration failure, conflict, non-Git backend, or another Git command
- **WHEN** Conflux classifies the failure
- **THEN** it does not retry under the final Apply index-lock policy
- **AND** it returns the original terminal failure context
