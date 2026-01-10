# cli Spec Delta

## ADDED Requirements

### Requirement: TUI Stop Processing with Escape Key

The TUI SHALL allow users to stop ongoing processing using the Escape key.

#### Scenario: First Esc press initiates graceful stop

- **WHEN** TUI is in Running mode
- **AND** an agent process is actively running
- **AND** user presses Escape key
- **THEN** the TUI transitions to Stopping mode
- **AND** header status displays "Stopping..." in yellow
- **AND** log displays "Stopping after current change completes..."
- **AND** current agent process continues to completion
- **AND** no new changes are picked up for processing

#### Scenario: Second Esc press forces immediate stop

- **WHEN** TUI is in Stopping mode
- **AND** user presses Escape key again
- **THEN** the current agent process is terminated immediately (SIGTERM)
- **AND** the TUI transitions to Stopped mode
- **AND** log displays "Force stopped - process terminated"
- **AND** the interrupted change status becomes "queued" (not error)

#### Scenario: Graceful stop completes naturally

- **WHEN** TUI is in Stopping mode
- **AND** the current agent process completes successfully
- **THEN** the TUI transitions to Stopped mode
- **AND** the completed change transitions to appropriate status (completed/archived)
- **AND** log displays "Stopped - processing halted"

#### Scenario: Esc has no effect in selection mode

- **WHEN** TUI is in Selecting mode
- **AND** user presses Escape key
- **THEN** nothing happens
- **AND** the TUI remains in Selecting mode

### Requirement: TUI Stopped Mode

The TUI SHALL provide a Stopped mode where users can review progress and manage the queue before resuming.

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

### Requirement: TUI Help Text for Stop

The TUI help text SHALL include stop key binding information.

#### Scenario: Running mode help text

- **WHEN** TUI is in Running mode
- **THEN** help text includes "Esc: stop"
- **AND** help text continues to show "q: quit"

#### Scenario: Stopping mode help text

- **WHEN** TUI is in Stopping mode
- **THEN** help text includes "Esc: force stop"
- **AND** help text shows "Waiting for current process..."

#### Scenario: Stopped mode help text

- **WHEN** TUI is in Stopped mode
- **THEN** help text includes "F5: resume"
- **AND** help text includes "Space: toggle queue"
- **AND** help text includes "q: quit"

### Requirement: Interrupted Change Handling

Changes interrupted by force stop SHALL be handled gracefully.

#### Scenario: Force-stopped change returns to queued

- **WHEN** a change is being processed
- **AND** user force stops with second Esc press
- **THEN** the change status becomes "queued" (not error)
- **AND** the change can be re-processed on resume
- **AND** no error message is displayed for the interruption

#### Scenario: Partial progress preserved

- **WHEN** a change had some tasks completed before force stop
- **THEN** the completed tasks remain completed
- **AND** the tasks.md file reflects actual progress
- **AND** resuming continues from the partial state

## MODIFIED Requirements

### Requirement: TUI Layout Configuration (MODIFIED)

The TUI SHALL display appropriate layout for Stopping and Stopped modes in addition to existing modes.

#### Scenario: Stopping mode layout

- **WHEN** TUI is in Stopping mode
- **THEN** header displays "Stopping..." status in yellow
- **AND** current processing panel shows "Completing..."
- **AND** log panel is visible with stop messages

#### Scenario: Stopped mode layout

- **WHEN** TUI is in Stopped mode
- **THEN** header displays "Stopped" status in gray
- **AND** status panel shows summary of completed/queued changes
- **AND** footer shows available actions (F5: resume, q: quit)
