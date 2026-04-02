## ADDED Requirements

### Requirement: Project state snapshots include sync-state metadata

Server project state APIs used by remote clients MUST expose the latest synchronization metadata for each registered project.

The exposed metadata MUST include at least:
- `sync_state`
- `ahead_count`
- `behind_count`
- `sync_required`
- `local_sha`
- `remote_sha`
- `last_remote_check_at`
- check error information when the latest refresh failed

#### Scenario: REST state snapshot includes sync-state metadata
- **GIVEN** a registered project has completed at least one remote sync-state check
- **WHEN** a client requests the server project state snapshot
- **THEN** the project payload includes `sync_state`, `ahead_count`, `behind_count`, `sync_required`, `local_sha`, `remote_sha`, and `last_remote_check_at`

#### Scenario: WebSocket full state includes sync-state metadata
- **GIVEN** a WebSocket client connects to the server state stream
- **WHEN** the server sends a `full_state` message
- **THEN** each project payload includes the latest sync-state metadata fields

#### Scenario: failed refresh exposes error details
- **GIVEN** the latest remote sync-state refresh failed for a registered project
- **WHEN** a client requests project state
- **THEN** the project payload includes the latest check failure details
- **AND** the payload still contains a valid `sync_state` field
