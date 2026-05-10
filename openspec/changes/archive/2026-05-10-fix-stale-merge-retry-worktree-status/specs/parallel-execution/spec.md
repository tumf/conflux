## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle stale or deleted retry worktree paths without running merge-readiness `git status` commands in missing directories. Archive-completion verification used for base merge readiness MUST use an existing repository root or fail before command execution with a bounded stale-retry outcome.

When a deferred merge retry no longer has a valid worktree path, the scheduler MUST derive the next action from repository-visible evidence: already merged changes are treated as completed, valid archived changes may be retried from an existing root, and changes with no valid retry evidence have retry intent cleared or suppressed with a single diagnostic.

#### Scenario: deleted deferred worktree does not run git status in missing cwd

- **GIVEN** change `alpha` is in reducer-owned `ResolveWait`
- **AND** the scheduler's discovered retry worktree path for `alpha` has been deleted
- **WHEN** deferred merge retry dispatch evaluates `alpha`
- **THEN** the scheduler SHALL NOT run `git status --porcelain` with cwd set to the deleted worktree path
- **AND** the retry outcome SHALL be derived from existing repository/base evidence
- **AND** the operator SHALL NOT see repeated `No such file or directory` merge-deferred warnings for the same stale path

#### Scenario: legitimate dirty base remains manual merge wait

- **GIVEN** change `alpha` has archived successfully
- **AND** the base repository root exists
- **AND** the base repository has uncommitted changes or an unresolved merge blocker before `alpha` can merge
- **WHEN** merge readiness is evaluated
- **THEN** the scheduler SHALL emit a manual `MergeDeferred(auto_resumable=false)` outcome
- **AND** `alpha` SHALL remain visible as `MergeWait`
- **AND** the diagnostic SHALL describe the actionable base blocker rather than a stale missing worktree

#### Scenario: stale retry converges after base integration

- **GIVEN** change `alpha` has already been integrated into the base branch
- **AND** a stale `ResolveWait` retry still references a deleted worktree path
- **WHEN** deferred merge retry dispatch evaluates `alpha`
- **THEN** the scheduler SHALL treat the retry as complete or stale-successful based on base evidence
- **AND** `alpha` SHALL NOT remain in an endless resolve-wait retry loop
