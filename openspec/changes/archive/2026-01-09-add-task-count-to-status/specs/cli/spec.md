## ADDED Requirements

### Requirement: Terminal Status Task Count Display

The TUI running mode SHALL display task counts embedded in the status text for terminal states (completed, archived, error) to provide at-a-glance progress information.

#### Scenario: Completed state shows task count
- **WHEN** a change is in `completed` status in running mode
- **THEN** the status text SHALL be displayed as `[completed X/Y]` where X is completed tasks and Y is total tasks
- **AND** the status is displayed in green color

#### Scenario: Archived state shows task count
- **WHEN** a change is in `archived` status in running mode
- **THEN** the status text SHALL be displayed as `[archived X/Y]` where X is completed tasks and Y is total tasks
- **AND** the status is displayed in blue color

#### Scenario: Error state shows task count
- **WHEN** a change is in `error` status in running mode
- **THEN** the status text SHALL be displayed as `[error X/Y]` where X is completed tasks and Y is total tasks
- **AND** the status is displayed in red color

#### Scenario: Processing state keeps progress percentage
- **WHEN** a change is in `processing` status in running mode
- **THEN** the status text SHALL continue to display progress percentage as `⠋ [ XX%]`
- **AND** task counts SHALL remain displayed in a separate column
