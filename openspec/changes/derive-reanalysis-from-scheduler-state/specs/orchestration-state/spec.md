## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

The parallel scheduler's decision to dispatch queued changes SHALL be derived from reducer-observable state (queue intent, active execution stage, available slots) rather than transient event flags. This ensures that changes with `QueueIntent::Queued` in the reducer are always considered for dispatch when execution capacity exists.

#### Scenario: Reducer queued change is visible to scheduler dispatch

- **GIVEN** a change has `QueueIntent::Queued` in the reducer
- **AND** no activity stage is active for that change
- **AND** available execution slots are greater than zero
- **WHEN** the scheduler evaluates dispatch candidates
- **THEN** the change is included in the re-analysis candidate set
- **AND** the scheduler does not require a separate event flag to consider this change
