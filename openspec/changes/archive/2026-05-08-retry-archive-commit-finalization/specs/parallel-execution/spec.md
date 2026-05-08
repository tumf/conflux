## ADDED Requirements

### Requirement: Archive commit finalization retries repairable failures

After a parallel archive command has successfully moved a change from `openspec/changes/<change_id>/` into `openspec/changes/archive/`, the runtime SHALL use a bounded archive commit finalization retry loop before returning terminal archive error.

Archive commit finalization SHALL include creation or verification of a clean `Archive: <change_id>` commit. Failures from git hooks, formatter hooks, clippy hooks, direct commit failures, final archive verification failures, or hook-modified files SHALL be treated as repairable until the finalization retry budget is exhausted.

The retry loop MUST NOT depend on durable workflow-control state outside the workspace. Retry decisions MUST be derived from workspace file state, workspace git state, base/archive verification, and in-memory attempt context from the current run.

#### Scenario: hook failure during archive commit is retried

- **GIVEN** parallel archive has moved `alpha` into `openspec/changes/archive/2026-05-08-alpha/`
- **AND** the first direct `Archive: alpha` commit fails because a pre-commit hook or clippy check fails
- **WHEN** archive commit finalization evaluates the failure
- **THEN** Conflux schedules another archive commit finalization attempt before returning terminal error
- **AND** the next attempt receives the previous hook stderr as context
- **AND** the change is not marked errored solely because the first archive commit attempt failed

#### Scenario: hook-modified files are restaged and retried

- **GIVEN** archive commit finalization runs `git commit -m "Archive: alpha"`
- **AND** a pre-commit hook modifies files and exits non-zero
- **WHEN** the finalization retry loop continues
- **THEN** Conflux re-checks `git status --porcelain`
- **AND** Conflux re-stages modified files before a later archive commit attempt
- **AND** finalization can succeed if the later attempt produces a clean `Archive: alpha` commit

#### Scenario: finalization resolve can fix compile or module errors

- **GIVEN** archive commit finalization fails with stderr showing a repairable source error such as an unresolved module import
- **WHEN** Conflux invokes a subsequent archive-finalization resolve attempt
- **THEN** the resolve prompt includes the prior stderr and current git status
- **AND** if the resolve attempt fixes the source error and creates a valid archive commit, archive completes successfully

#### Scenario: archive command is not rerun when only commit finalization is incomplete

- **GIVEN** archive file movement has already succeeded for `alpha`
- **AND** only the final archive commit remains incomplete
- **WHEN** the finalization retry loop runs
- **THEN** Conflux retries archive commit finalization rather than re-running the full archive command unnecessarily
- **AND** it still revalidates that the active change directory remains absent and the archive entry remains present

#### Scenario: terminal archive error waits for finalization retry exhaustion

- **GIVEN** archive commit finalization repeatedly fails for `alpha`
- **AND** the bounded finalization retry budget is exhausted
- **WHEN** Conflux reports terminal archive failure
- **THEN** the error identifies archive commit finalization as the failed phase
- **AND** the error includes the last actionable blocker from direct commit, hook stderr, resolve output, or archive completion verification

#### Scenario: finalization retry events are visible

- **GIVEN** archive commit finalization needs another attempt after a failed commit or failed verification
- **WHEN** the retry is scheduled
- **THEN** Conflux emits a user-visible log or event that distinguishes archive commit finalization retry from archive command retry
- **AND** the event includes the attempt number, bounded retry limit, and a concise reason
