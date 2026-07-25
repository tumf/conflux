## MODIFIED Requirements

### Requirement: Serial run resolves workflow state from its captured repository root

Serial orchestration MUST rediscover changes after apply and evaluate subsequent acceptance, archive, and resume routing relative to the repository root captured when the service was created. Ambient process working-directory changes MUST NOT redirect those operations to another repository.

Serial orchestration MUST NOT create or load `.cflx/acceptance-state.json`. Within one active run it MAY retain acceptance context in memory. After restart, complete unarchived work MUST run acceptance again before archive unless repository evidence already proves archive or base integration.

#### Scenario: ambient working directory does not redirect serial resume

- **GIVEN** serial run was created for repository `alpha`
- **AND** the process working directory later points at repository `beta`
- **WHEN** serial run refreshes the change after apply or evaluates resume routing
- **THEN** it reads repository `alpha`
- **AND** repository `beta` does not influence workflow routing
- **AND** neither repository receives `.cflx/acceptance-state.json`

#### Scenario: serial restart reruns acceptance for unarchived work

- **GIVEN** repository `alpha` contains a complete implementation that is not repository-verifiably archived
- **AND** the previous serial process ended after acceptance activity
- **WHEN** a new serial run resumes repository `alpha`
- **THEN** it runs acceptance before archive
- **AND** it does not infer PASS from a generated checkpoint

#### Scenario: serial uninterrupted pass hands off in memory

- **GIVEN** serial apply and acceptance execute in one active run
- **WHEN** acceptance returns PASS for the current revision
- **THEN** serial execution may continue to archive using active-run context
- **AND** no acceptance JSON checkpoint is written
