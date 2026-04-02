## ADDED Requirements

### Requirement: Force stop and dequeue returns a running change to not queued

The system SHALL support a force-stop-and-dequeue operation for a running change.

This operation MUST cancel the in-flight execution for the target change and, once cancellation is confirmed, clear the reducer-owned runtime state back to a non-terminal idle queue-off state.

After the operation completes, the target change MUST satisfy all of the following:

- `queue_intent` is `NotQueued`
- `activity` is `Idle`
- `wait_state` is `None`
- `terminal` is `None`
- the derived display status is `not queued`

The force-stop-and-dequeue operation MUST be distinct from terminal stop semantics such as `Stopped`, and MUST NOT leave the change in a terminal stopped state.

#### Scenario: Running apply is force-stopped and dequeued

- **GIVEN** a change is currently in an active execution stage such as `Applying`
- **WHEN** the user invokes force-stop-and-dequeue for that change
- **THEN** the in-flight execution is cancelled
- **AND** after cancellation confirmation the reducer clears the change to `NotQueued` + `Idle` + `None wait` + `None terminal`
- **AND** the derived display status is `not queued`

#### Scenario: Stale stop completion does not create terminal stopped state

- **GIVEN** a running change has already completed force-stop-and-dequeue
- **WHEN** a late stop-related event from the cancelled worker arrives
- **THEN** the reducer ignores any regression to terminal `Stopped`
- **AND** the derived display status remains `not queued`

### Requirement: Force-stop-and-dequeue does not auto-resume work

After a change has been force-stopped and dequeued, the system SHALL NOT automatically re-queue or restart that change unless the user explicitly requests queueing again.

#### Scenario: Refresh does not re-queue dequeued change

- **GIVEN** a change has completed force-stop-and-dequeue and currently displays `not queued`
- **WHEN** the system processes a later refresh or reconciliation pass
- **THEN** the reducer preserves `NotQueued`
- **AND** the change does not transition back to an active or queued state without a new explicit queue command

### Requirement: Force-stop-and-dequeue only applies to retryable active work

The system SHALL apply force-stop-and-dequeue only to changes that are currently retryable and in-flight or queued for in-flight cancellation handling.

The operation MUST NOT convert permanent terminal changes such as `Archived`, `Merged`, or `Rejected` into `not queued`.

#### Scenario: Archived change ignores force-stop-and-dequeue

- **GIVEN** a change is already in terminal `Archived`
- **WHEN** force-stop-and-dequeue is requested for that change
- **THEN** the reducer treats the request as a no-op
- **AND** the derived display status remains `archived`
