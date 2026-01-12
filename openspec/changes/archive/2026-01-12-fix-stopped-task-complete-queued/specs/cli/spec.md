# cli Specification Delta

## MODIFIED Requirements

### Requirement: TUI Stopped Mode

The TUI SHALL provide a Stopped mode where users can review progress and manage the queue before resuming. Task completion in Stopped mode SHALL NOT automatically add changes to the queue.

#### Scenario: Stopped mode display

- **WHEN** TUI is in Stopped mode
- **THEN** header status displays "Stopped" in gray color
- **AND** the change list remains visible with current statuses
- **AND** completed changes show "[completed]" or "[archived]"
- **AND** remaining queued changes show "queued"

#### Scenario: Queue management in Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a queued change
- **THEN** the change is removed from queue (becomes not queued)

#### Scenario: Queue addition in Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-queued change
- **THEN** the change is added to the queue

#### Scenario: Task completion in Stopped mode does not auto-queue

- **WHEN** TUI is in Stopped mode
- **AND** a change's tasks are updated (e.g., all tasks marked complete)
- **THEN** the change queue_status SHALL remain unchanged
- **AND** the change SHALL NOT be automatically added to the queue
- **AND** the change SHALL only be queued when user explicitly presses Space key

#### Scenario: Resume processing from Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** one or more changes are queued
- **AND** user presses F5
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes with the queued changes
- **AND** log displays "Resuming processing..."

#### Scenario: Resume with empty queue shows warning

- **WHEN** TUI is in Stopped mode
- **AND** no changes are queued
- **AND** user presses F5
- **THEN** a warning message is displayed
- **AND** the TUI remains in Stopped mode
