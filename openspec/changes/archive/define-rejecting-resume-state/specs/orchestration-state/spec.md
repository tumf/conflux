## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

The active execution stage SHALL include lifecycle events for entering and leaving the `Rejecting` stage. The reducer SHALL support two outcomes from rejection review: confirmation (transition to `Rejected` terminal) and dismissal/resume (transition back to `Applying`).

#### Scenario: Rejection review confirm transitions to rejected terminal

- **GIVEN** a change is in `Rejecting` activity stage
- **WHEN** the reducer applies a rejection-review-completed event with `Confirm` outcome
- **THEN** the activity becomes `Idle`
- **AND** the terminal state becomes `Rejected`
- **AND** the derived display status is `rejected`

#### Scenario: Rejection review resume transitions back to applying

- **GIVEN** a change is in `Rejecting` activity stage
- **WHEN** the reducer applies a rejection-review-completed event with `Resume` outcome
- **THEN** the activity becomes `Applying`
- **AND** the terminal state remains `None`
- **AND** the derived display status is `applying`

#### Scenario: Rejection review failure transitions to error terminal

- **GIVEN** a change is in `Rejecting` activity stage
- **WHEN** the reducer applies a rejection-review-failed event
- **THEN** the activity becomes `Idle`
- **AND** the terminal state becomes `Error`
- **AND** the derived display status is `error`
