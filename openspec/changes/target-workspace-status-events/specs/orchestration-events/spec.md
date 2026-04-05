## MODIFIED Requirements

### Requirement: Execution events identify their target change explicitly

Execution events that mutate per-change runtime activity SHALL identify the target change explicitly in the event payload. Events used to synchronize workspace activity from parallel execution MUST be applicable without consulting unrelated orchestrator-global cursors such as `current_change_id`.

#### Scenario: Workspace status update names its target change

- **GIVEN** a parallel workspace transitions into `Rejecting`
- **WHEN** the runtime emits a workspace status synchronization event
- **THEN** the event payload includes the target change identifier
- **AND** downstream reducers can update the matching runtime entry directly
