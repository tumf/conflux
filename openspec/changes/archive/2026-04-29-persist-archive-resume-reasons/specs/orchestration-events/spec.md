## ADDED Requirements

### Requirement: Archive retry and resume events identify retry cause explicitly

Execution events used to synchronize archive retry or archive resume activity SHALL identify the target change and the archive retry/resume cause explicitly.

#### Scenario: archive retry event includes target change and reason

- **GIVEN** a parallel workspace for change `delta` schedules another archive attempt
- **WHEN** the runtime emits the archive retry synchronization event
- **THEN** the event payload includes the target change identifier
- **AND** the payload includes the archive primary reason and summary
- **AND** downstream reducers or UI layers can render why archive is looping without consulting unrelated global cursors or parsing free-form log text only
