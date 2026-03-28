## MODIFIED Requirements

### Requirement: Dynamic Queue Management

The system SHALL provide the ability to dynamically add and remove changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

When `AllCompleted` is received while any change is still in `Resolving` status, the system SHALL keep `AppMode::Running` instead of transitioning to `AppMode::Select`. This ensures users can continue to add changes to the queue via Space key during resolve.

When the last active change (including Resolving) completes or fails, and no other active changes remain, the system SHALL transition to `AppMode::Select`.

When the user triggers Stop, the system SHALL reset `Resolving` changes to `NotQueued` alongside other active statuses.

#### Scenario: Add to queue during execution
- **WHEN** the user selects a change with the Space key in Running mode
- **AND** the change is in NotQueued status
- **THEN** the system SHALL emit an AddToQueue command

#### Scenario: AllCompleted with active resolve preserves Running mode
- **GIVEN** one or more changes are in `Resolving` status
- **WHEN** the `AllCompleted` event is received
- **THEN** `AppMode` SHALL remain `Running`
- **AND** the user can still add changes to the queue via Space key

#### Scenario: Resolve completion triggers Select when no active changes remain
- **GIVEN** the `AllCompleted` event was previously received while resolving
- **AND** the last `Resolving` change completes or fails
- **WHEN** no other changes have an active queue status
- **THEN** `AppMode` SHALL transition to `Select`

#### Scenario: Stop resets Resolving changes
- **GIVEN** one or more changes are in `Resolving` status
- **WHEN** the user triggers Stop
- **THEN** the `Resolving` changes SHALL be reset to `NotQueued`
