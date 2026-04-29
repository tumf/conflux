## ADDED Requirements

### Requirement: Archive retry and resume reasons persist across process restarts

Parallel archive execution SHALL persist the primary archive retry/resume reason outside the worktree so that an interrupted or restarted runtime can explain why archive is being retried or resumed.

#### Scenario: archiving workspace restores prior archive reason on resume

- **GIVEN** change `alpha` previously entered archive and recorded a durable archive failure state with primary reason `verification_failed`
- **AND** the workspace file state is later detected as `Archiving`
- **WHEN** the runtime resumes processing after process restart
- **THEN** the archive resume context includes the previously recorded primary reason and summary
- **AND** the next archive retry path can surface that reason to logs, events, or agent context without depending on in-memory history from the prior process

### Requirement: Archived workspaces remain terminal even when durable archive state exists

Durable archive retry/resume state MUST NOT cause an already archived workspace to re-enter apply, acceptance, or archive.

#### Scenario: archived workspace ignores stale retry state and goes to merge handoff

- **GIVEN** change `beta` has a durable archive retry state from an earlier failed archive attempt
- **AND** current workspace file state is detected as `Archived`
- **WHEN** resume routing is performed
- **THEN** the runtime treats the workspace as terminal for archive purposes and hands it off to merge handling
- **AND** the stale durable archive retry state does not route the change back into apply, acceptance, or archive

### Requirement: Archive retry observability includes a structured primary reason

When archive is retried, resumed, or fails terminally, the runtime SHALL expose a structured primary reason plus supplemental context rather than only a generic retry/failure message.

#### Scenario: archive retry log/event names the retry reason

- **GIVEN** change `gamma` fails archive verification because the change directory still exists after the archive attempt
- **WHEN** the runtime schedules another archive retry
- **THEN** the retry log or event payload includes a primary archive reason indicating verification failure
- **AND** the payload includes a summary describing the concrete symptom
- **AND** downstream consumers do not have to infer the reason only from a generic `retrying archive command` string
