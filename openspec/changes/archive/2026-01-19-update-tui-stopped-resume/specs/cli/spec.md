## MODIFIED Requirements

### Requirement: TUI Stopped Mode
The TUI SHALL provide a Stopped mode that manages change state by holding queued status only during execution. When transitioning to Stopped, queue_status SHALL be reset to NotQueued while preserving execution marks ([x]). Space operations in Stopped mode SHALL only add/remove execution marks while maintaining queue_status as NotQueued. When resuming with F5, execution-marked changes SHALL be restored to queued and processing SHALL resume. Task progress updates in Stopped mode SHALL NOT trigger queuing.

#### Scenario: Stopped mode display
- **WHEN** TUI is in Stopped mode
- **THEN** header status displays "Stopped" in gray color
- **AND** the change list remains visible with current statuses
- **AND** execution-marked changes show "[x]" while their queue_status remains not queued

#### Scenario: Queue management in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on an execution-marked change
- **THEN** the execution mark is removed and queue_status remains not queued

#### Scenario: Queue addition in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-marked change
- **THEN** the execution mark is added and queue_status remains not queued

#### Scenario: Task completion in Stopped mode does not auto-queue
- **WHEN** TUI is in Stopped mode
- **AND** a change's tasks are updated (e.g., all tasks marked complete)
- **THEN** the change queue_status SHALL remain not queued
- **AND** the change SHALL NOT be automatically added to the queue

#### Scenario: Resume processing from Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** one or more changes are execution-marked
- **AND** user presses F5
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes after converting execution-marked changes to queued
- **AND** log displays "Resuming processing..."

#### Scenario: Resume with empty queue shows warning
- **WHEN** TUI is in Stopped mode
- **AND** no changes are execution-marked
- **AND** user presses F5
- **THEN** a warning message is displayed
- **AND** the TUI remains in Stopped mode

### Requirement: Interrupted Change Handling
Changes interrupted by stop SHALL be handled according to the policy of holding queued status only during execution. When force-stopped, queue_status SHALL be reset to NotQueued while preserving execution marks. On resume, execution-marked changes SHALL be restored to queued and can be re-processed.

#### Scenario: Force-stopped change returns to not queued
- **WHEN** a change is being processed
- **AND** user force stops with second Esc press
- **THEN** the change status becomes not queued (not error)
- **AND** the execution mark remains set
- **AND** the change can be re-processed on resume

#### Scenario: Partial progress preserved
- **WHEN** a change had some tasks completed before force stop
- **THEN** the completed tasks remain completed
- **AND** the tasks.md file reflects actual progress
- **AND** resuming continues from the partial state
