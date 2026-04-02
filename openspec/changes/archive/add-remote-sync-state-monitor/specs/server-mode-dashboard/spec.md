## ADDED Requirements

### Requirement: Server-mode dashboard shows per-project sync state

The server-mode dashboard SHALL render each project's remote synchronization state using display-ready metadata from the server rather than inferring status from logs.

#### Scenario: project list shows behind state
- **GIVEN** a project payload reports `sync_state = behind` and `behind_count > 0`
- **WHEN** the dashboard renders the project list
- **THEN** the project row indicates that the remote branch is ahead of local
- **AND** the row shows the behind count in a display-ready form

#### Scenario: project list shows ahead state
- **GIVEN** a project payload reports `sync_state = ahead` and `ahead_count > 0`
- **WHEN** the dashboard renders the project list
- **THEN** the project row indicates that the local branch is ahead of remote
- **AND** the row shows the ahead count in a display-ready form

#### Scenario: project list shows diverged state
- **GIVEN** a project payload reports `sync_state = diverged`
- **WHEN** the dashboard renders the project list
- **THEN** the project row indicates that local and remote have diverged
- **AND** the row can display both ahead and behind counts

#### Scenario: project list shows unknown state after check failure
- **GIVEN** a project payload reports `sync_state = unknown`
- **WHEN** the dashboard renders the project list
- **THEN** the project row indicates that sync state could not be determined
- **AND** the dashboard can surface the latest check failure message when available
