## MODIFIED Requirements

### Requirement: UI State Persistence

The server SHALL persist dashboard UI preferences in the `ui_state` table as key-value pairs and expose them via REST API and the FullState WebSocket payload. The persisted UI-state surface SHALL be generic enough to store reload-restorable dashboard selection context, including the selected project, proposal session, file-browse target, and auxiliary tab preferences needed to re-surface a restored selection.

#### Scenario: Store and retrieve UI state

- **GIVEN** the server is running with a SQLite database
- **WHEN** a client calls `PUT /api/v1/ui-state/selected_project_id` with body `{"value": "proj-1"}`
- **THEN** the value is stored in the `ui_state` table and subsequent `GET /api/v1/ui-state` returns `{"selected_project_id": "proj-1"}`

#### Scenario: Store file browse context

- **GIVEN** the server is running with a SQLite database
- **WHEN** a client calls `PUT /api/v1/ui-state/file_browse_context` with a serialized value representing the current change or worktree selection
- **THEN** the value is stored in the `ui_state` table without schema changes specific to that UI-state key
- **AND** subsequent `GET /api/v1/ui-state` returns the same serialized value under `file_browse_context`

#### Scenario: Delete UI state key

- **GIVEN** a UI state key `active_proposal_session_id` exists
- **WHEN** a client calls `DELETE /api/v1/ui-state/active_proposal_session_id`
- **THEN** the key is removed from the `ui_state` table

#### Scenario: FullState includes ui_state

- **GIVEN** UI state keys exist in the database
- **WHEN** the server sends a FullState WebSocket message
- **THEN** the message includes a `ui_state` object with all stored key-value pairs
