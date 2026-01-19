## MODIFIED Requirements
### Requirement: Change Selection Mode

At TUI launch, the change selection mode SHALL be displayed, allowing users to select changes for processing.

#### Scenario: Exit
- **WHEN** user presses `Ctrl+C`
- **THEN** TUI exits and the terminal is restored to its original state

### Requirement: Running Mode Dashboard

TUI SHALL display a dashboard-style UI in running mode.

#### Scenario: Display on processing completion
- **WHEN** all queued changes have been processed
- **THEN** the header status changes to "Completed"
- **AND** "Done" is displayed in green on the left side of the status panel
- **AND** TUI maintains display, allowing user to exit with `Ctrl+C`

#### Scenario: Queue modification after completion
- **WHEN** AppMode is Completed
- **AND** user presses Space key
- **THEN** NotQueued changes can be changed to Queued
- **AND** Queued changes can be changed to NotQueued
- **AND** Completed/Archived/Error changes cannot be modified

#### Scenario: Re-run after completion
- **WHEN** AppMode is Completed
- **AND** changes are added to the queue
- **AND** user presses F5 key
- **THEN** AppMode changes to Running
- **AND** processing of queued changes begins
