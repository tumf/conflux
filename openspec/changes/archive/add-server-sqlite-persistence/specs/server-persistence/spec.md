## ADDED Requirements

### Requirement: Server SQLite Database

The server daemon SHALL initialize a SQLite database at `<data_dir>/cflx.db` on startup, applying schema migrations via `PRAGMA user_version`.

#### Scenario: Database initialization on first start

- **GIVEN** a server data directory with no existing `cflx.db`
- **WHEN** the server starts
- **THEN** a new SQLite database is created with all required tables and WAL mode enabled

#### Scenario: Schema migration on upgrade

- **GIVEN** a server data directory with an older `cflx.db` (lower `user_version`)
- **WHEN** the server starts
- **THEN** schema migrations are applied incrementally to bring the database to the current version

### Requirement: Change Event Persistence

The server SHALL persist all change processing events (apply, archive, acceptance, resolve) to the `change_events` table.

#### Scenario: Apply attempt recorded

- **GIVEN** a running server processing a change
- **WHEN** an apply attempt completes (success or failure)
- **THEN** a row is inserted into `change_events` with operation='apply', attempt number, success flag, duration, exit code, and output tails

#### Scenario: Events queryable by project and change

- **GIVEN** change events stored in the database
- **WHEN** `GET /api/v1/stats/projects/:id/history` is called
- **THEN** the response contains the change events for that project ordered by creation time

### Requirement: Log Entry Persistence

The server SHALL persist log entries to the `log_entries` table and provide a query API.

#### Scenario: Log entries stored

- **GIVEN** a running server
- **WHEN** a log entry is broadcast via WebSocket
- **THEN** the same entry is inserted into `log_entries`

#### Scenario: Log query with pagination

- **GIVEN** stored log entries
- **WHEN** `GET /api/v1/logs?project_id=X&limit=50&before=<iso8601>` is called
- **THEN** the response contains up to 50 log entries for project X created before the given timestamp

### Requirement: Log Rotation

The server SHALL automatically delete log entries older than 30 days.

#### Scenario: Old logs cleaned up on startup

- **GIVEN** log entries older than 30 days exist
- **WHEN** the server starts
- **THEN** those log entries are deleted from the database

### Requirement: Change State Persistence

The server SHALL persist change selection and error states to the `change_states` table and restore them on startup.

#### Scenario: Selection state survives restart

- **GIVEN** a change has been deselected via toggle
- **WHEN** the server restarts
- **THEN** the change remains deselected

#### Scenario: Error state survives restart

- **GIVEN** a change has been marked as errored
- **WHEN** the server restarts
- **THEN** the change remains in error state with its error message preserved

### Requirement: Statistics Overview API

The server SHALL provide `GET /api/v1/stats/overview` returning aggregate statistics across all projects.

#### Scenario: Overview includes success and failure counts

- **GIVEN** change events exist for multiple projects
- **WHEN** `GET /api/v1/stats/overview` is called
- **THEN** the response includes total success count, total failure count, and average duration per operation type
