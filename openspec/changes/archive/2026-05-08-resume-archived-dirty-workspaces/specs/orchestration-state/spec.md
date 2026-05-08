## ADDED Requirements

### Requirement: Archived dirty workspaces remain scheduler-recoverable after archive finalization failure

When a parallel workspace has already moved a change into `openspec/changes/archive/` but the archive commit is still incomplete, the runtime SHALL treat that state as recoverable scheduler-owned work rather than as permanently terminal solely because a prior run emitted an archive failure.

Recovery decisions SHALL be derived from repository-visible workspace state, including active change path absence, archive path presence, incomplete archive commit verification, and current git state. The system MUST NOT require durable external resume state to rediscover the workspace.

The scheduler SHALL be able to re-own and resume archive finalization repair for such an archived dirty workspace on a later cycle or restarted run, unless the bounded recovery policy has been exhausted for the current attempted repair path and the workspace is explicitly classified as terminal.

#### Scenario: archived dirty workspace is reclaimed on later scheduler cycle

- **GIVEN** change `alpha` has been moved to `openspec/changes/archive/2026-05-08-alpha/` in its workspace
- **AND** `openspec/changes/alpha/` no longer exists in that workspace
- **AND** the workspace still lacks a complete `Archive: alpha` commit
- **WHEN** a later scheduler cycle inspects repository-visible workspace state
- **THEN** Conflux reclaims `alpha` as archive-finalization recovery work
- **AND** the scheduler does not remain idle while that recoverable work exists

#### Scenario: archived dirty recovery does not require full archive command rerun

- **GIVEN** archive file movement for `alpha` is already correct
- **AND** only archive commit finalization remains incomplete
- **WHEN** Conflux resumes recovery for `alpha`
- **THEN** it resumes archive finalization repair rather than re-running the full archive command unnecessarily
- **AND** it still verifies that archive file-state has not regressed

#### Scenario: archive move regression re-enters full archive path

- **GIVEN** a previously archived dirty workspace for `alpha`
- **AND** later inspection shows the archive entry is missing or the active change directory has reappeared
- **WHEN** Conflux evaluates recovery
- **THEN** it does not treat the workspace as archive-finalization-only recovery
- **AND** it may require the broader archive path again based on current file state

#### Scenario: archived dirty state is distinct from terminal archive failure

- **GIVEN** a run previously emitted `Archive commit verification failed` for `alpha`
- **AND** the workspace still shows archive files present and commit incomplete
- **WHEN** Conflux derives current runtime state from the workspace
- **THEN** it exposes a recoverable archived-dirty/archive-finalization-needed state instead of only terminal archive failure
- **AND** user-visible events/logs distinguish that recoverable state from exhausted terminal failure

#### Scenario: exhausted archive-finalization recovery becomes terminal

- **GIVEN** archived dirty recovery for `alpha` has exhausted its bounded retry policy
- **WHEN** Conflux reports the final outcome
- **THEN** it MAY emit a terminal archive failure
- **AND** the reported blocker identifies the final archive-finalization reason rather than implying the archive move itself never happened
