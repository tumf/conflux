## MODIFIED Requirements

### Requirement: Scheduler dispatch derives queued candidates from reducer state

The parallel scheduler's decision to dispatch queued changes SHALL be derived from reducer-observable state (queue intent, active execution stage, available slots) rather than transient event flags. This ensures that changes with `QueueIntent::Queued` in the reducer are always considered for dispatch when execution capacity exists.

The scheduler SHALL reconcile reducer-visible queued intent into its scheduler-local candidate set before declaring the local queue empty, before exiting due to drained work, and before skipping re-analysis solely because the local queue is empty. Dynamic queue notifications MAY wake the scheduler, but they MUST NOT be the only mechanism by which reducer-queued work becomes eligible for analysis.

#### Scenario: Reducer queued change is visible to scheduler dispatch

- **GIVEN** a change has `QueueIntent::Queued` in the reducer
- **AND** no activity stage is active for that change
- **AND** available execution slots are greater than zero
- **WHEN** the scheduler evaluates dispatch candidates
- **THEN** the change is included in the re-analysis candidate set
- **AND** the scheduler does not require a separate event flag to consider this change

#### Scenario: Local queued vector cannot hide reducer queued intent

- **GIVEN** a change has `QueueIntent::Queued` in the reducer
- **AND** the scheduler-local queued vector does not include that change
- **AND** the change is loadable from active OpenSpec change state
- **AND** no terminal or active state makes the change ineligible
- **WHEN** the scheduler evaluates whether work is drained or analysis should run
- **THEN** the scheduler reconciles the change into its local candidate set
- **AND** the scheduler does not exit or sleep indefinitely solely because its pre-reconcile local queued vector was empty

#### Scenario: Queued intent skip reason is observable

- **GIVEN** a change has `QueueIntent::Queued` in the reducer
- **AND** scheduler reconciliation does not add it to analysis candidates
- **WHEN** the scheduler records the reconciliation result
- **THEN** the reason is observable as a log or event
- **AND** the reason distinguishes at least active/in-flight, terminal, not loadable, no available slots, and debounce-delayed cases when those cases apply
