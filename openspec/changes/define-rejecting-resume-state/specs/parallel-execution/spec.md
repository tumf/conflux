## MODIFIED Requirements

### Requirement: ParallelRunService rejection flow on blocked execution

After rejecting review completes, the runtime SHALL emit a `RejectionReviewCompleted` execution event with either `Confirm` or `Resume` outcome. The reducer SHALL use this event to drive the `Rejecting → Rejected` or `Rejecting → Applying` transition.

The runtime SHALL NOT leave a change in the `Rejecting` activity stage after rejection review has produced a verdict. If rejection review encounters an error, the runtime SHALL emit a `RejectionReviewFailed` event to transition the change to `Error` terminal state.

#### Scenario: confirmed rejection emits completion event

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: CONFIRM`
- **WHEN** the rejection flow completes
- **THEN** a `RejectionReviewCompleted` event with `Confirm` outcome is emitted
- **AND** the reducer transitions the change to `Rejected` terminal state

#### Scenario: resumed rejection emits completion event and returns to apply

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: RESUME`
- **WHEN** the resume flow completes
- **THEN** a `RejectionReviewCompleted` event with `Resume` outcome is emitted
- **AND** the reducer transitions the change back to `Applying` activity
