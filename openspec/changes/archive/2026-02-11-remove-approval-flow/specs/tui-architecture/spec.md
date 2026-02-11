## MODIFIED Requirements

### Requirement: Dynamic Queue Management
The system SHALL provide the ability to dynamically add and remove changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

#### Scenario: Add to queue during execution
- **WHEN** the user selects a change with the Space key in Running mode
- **THEN** the change ID is added to DynamicQueue and will be executed in the next processing cycle

#### Scenario: Remove from queue during execution
- **WHEN** the user changes a [x] change to [ ] with the Space key in Running mode
- **THEN** the corresponding change ID is removed from DynamicQueue and will not be executed

#### Scenario: Prevent duplicate additions
- **WHEN** attempting to add a change ID that already exists in the queue
- **THEN** the addition is rejected and the queue state remains unchanged

#### Scenario: Remove non-existent ID
- **WHEN** attempting to remove a change ID that does not exist in the queue
- **THEN** no error occurs and the queue state remains unchanged

### Requirement: Queue State Synchronization

The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for resolve completion, and Space queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for queue operations.

However, for `ResolveWait`/`MergeWait` rows, the following SHALL be satisfied:
- Space operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.
- @ operation SHALL be ignored and MUST NOT modify any state.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is not a target for queue operations.

#### Scenario: Remove from queue with Space key
- **WHEN** the user dequeues a [x] change with the Space key in Running mode
- **THEN** the status changes to `QueueStatus::NotQueued` and is removed from DynamicQueue

#### Scenario: Log removal operations
- **WHEN** a change is removed from DynamicQueue
- **THEN** the removal operation is logged

#### Scenario: Cannot change queue state during ResolveWait
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `ResolveWait`
- **WHEN** the user presses Space or `@`
- **THEN** the change status SHALL remain `ResolveWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space operation toggles only the execution mark

#### Scenario: Cannot change queue state during MergeWait
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait`
- **WHEN** the user presses Space or `@`
- **THEN** the change status SHALL remain `MergeWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space operation toggles only the execution mark
