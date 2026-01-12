# cli Specification Delta

## MODIFIED Requirements

### Requirement: Queue management in Stopped mode

Task completion in Stopped mode SHALL NOT automatically add changes to the queue. Changes SHALL only be queued through explicit user action (Space key).

#### Scenario: Task completion in Stopped mode does not auto-queue

- **WHEN** TUI is in Stopped mode
- **AND** a change's tasks are updated (e.g., all tasks marked complete)
- **THEN** the change queue_status SHALL remain unchanged
- **AND** the change SHALL NOT be automatically added to the queue
- **AND** the change SHALL only be queued when user explicitly presses Space key

#### Scenario: Explicit queue addition in Stopped mode works

- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-queued change (even if tasks are 100% complete)
- **THEN** the change SHALL be added to the queue
- **AND** the change queue_status SHALL become Queued
