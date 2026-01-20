## MODIFIED Requirements

### Requirement: Terminal Status Task Count Display

TUI running mode SHALL display terminal states with status-only text and task counts in a separate column, avoiding redundant display.

#### Scenario: Archived state display format
- **WHEN** a change is in `archived` status in running mode
- **THEN** the status text SHALL be displayed as `[archived]` (without task count)
- **AND** the status is displayed in blue color
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Error state display format
- **WHEN** a change is in `error` status in running mode
- **THEN** the status text SHALL be displayed as `[error]` (without task count)
- **AND** the status is displayed in red color
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Processing state keeps progress percentage with task count
- **WHEN** a change is in `processing` status in running mode
- **THEN** the status text SHALL continue to display progress percentage as `⠋ [ XX%]`
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Terminal states skip completed and archive immediately
- **WHEN** a change reaches completion criteria for apply and acceptance
- **THEN** the queue status transitions directly to `archiving` without using `completed`
- **AND** no execution path leaves the change in a `completed` state (completed is never observable or durable)
- **AND** the change SHALL proceed to `archiving` immediately without pausing in any intermediate state or waiting for manual action
- **AND** completed MUST NOT be emitted as a queue status at any point
- **AND** completion always results in archiving without manual intervention
- **AND** no hook or retry flow can keep a change in completed state
- **AND** there is no configuration option that re-enables completed state
- **AND** failure to archive is handled as an error, not as a completed state
- **AND** the running mode status text SHALL not display `[completed]`
