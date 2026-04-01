## ADDED Requirements

### Requirement: UI State Persistence

The server SHALL persist dashboard UI preferences in the `ui_state` table as key-value pairs and expose them via REST API and the FullState WebSocket payload.

#### Scenario: Store and retrieve UI state

- **GIVEN** the server is running with a SQLite database
- **WHEN** a client calls `PUT /api/v1/ui-state/selected_project_id` with body `{"value": "proj-1"}`
- **THEN** the value is stored in the `ui_state` table and subsequent `GET /api/v1/ui-state` returns `{"selected_project_id": "proj-1"}`

#### Scenario: Delete UI state key

- **GIVEN** a UI state key `active_proposal_session_id` exists
- **WHEN** a client calls `DELETE /api/v1/ui-state/active_proposal_session_id`
- **THEN** the key is removed from the `ui_state` table

#### Scenario: FullState includes ui_state

- **GIVEN** UI state keys exist in the database
- **WHEN** the server sends a FullState WebSocket message
- **THEN** the message includes a `ui_state` object with all stored key-value pairs

### Requirement: Proposal Session Persistence

The server SHALL persist proposal session metadata to the `proposal_sessions` table and restore surviving sessions on startup.

#### Scenario: Session persisted on creation

- **GIVEN** a new proposal session is created
- **WHEN** the ACP subprocess is successfully spawned
- **THEN** the session metadata (id, project_id, worktree_path, worktree_branch, status, acp_session_id, timestamps) is inserted into the `proposal_sessions` table

#### Scenario: Session restored on server restart

- **GIVEN** the server restarts and the `proposal_sessions` table contains an active session whose worktree path exists on disk
- **WHEN** the server initializes the ProposalSessionManager
- **THEN** the session is restored with a re-spawned ACP subprocess and its message history loaded from the database

#### Scenario: Stale session cleaned up on restart

- **GIVEN** the server restarts and the `proposal_sessions` table contains a session whose worktree path no longer exists
- **WHEN** the server initializes the ProposalSessionManager
- **THEN** the session row and its messages are deleted from the database

#### Scenario: Session removed on close or merge

- **GIVEN** an active proposal session exists in the database
- **WHEN** the session is closed or merged
- **THEN** the session row and all associated message rows are deleted from the database

### Requirement: Proposal Session Message Persistence

The server SHALL persist proposal session chat messages to the `proposal_session_messages` table for history restoration.

#### Scenario: User message persisted

- **GIVEN** an active proposal session
- **WHEN** a user prompt is recorded via `record_user_prompt`
- **THEN** the message is inserted into the `proposal_session_messages` table

#### Scenario: Assistant message persisted on turn complete

- **GIVEN** an active proposal session with an ongoing assistant turn
- **WHEN** `complete_active_turn` is called
- **THEN** the accumulated assistant message (including tool_calls as JSON) is inserted into the `proposal_session_messages` table

#### Scenario: Messages loaded on session restoration

- **GIVEN** a proposal session is being restored from the database
- **WHEN** the server loads the session
- **THEN** all messages for that session are loaded from `proposal_session_messages` ordered by seq and populated into `ProposalSession.message_history`

## MODIFIED Requirements

### Requirement: Server SQLite Database

The server daemon SHALL initialize a SQLite database at `<data_dir>/cflx.db` on startup, applying schema migrations via `PRAGMA user_version`.

#### Scenario: Database initialization on first start

- **GIVEN** a server data directory with no existing `cflx.db`
- **WHEN** the server starts
- **THEN** a new SQLite database is created with all required tables (including `ui_state`, `proposal_sessions`, `proposal_session_messages`) and WAL mode enabled

#### Scenario: Schema migration on upgrade

- **GIVEN** a server data directory with an older `cflx.db` (lower `user_version`)
- **WHEN** the server starts
- **THEN** schema migrations are applied incrementally to bring the database to the current version (version 2 adds `ui_state`, `proposal_sessions`, and `proposal_session_messages` tables)
