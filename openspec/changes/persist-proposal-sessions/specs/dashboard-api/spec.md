## ADDED Requirements

### Requirement: UI State REST API

The server SHALL provide REST endpoints to read, write, and delete dashboard UI state preferences stored in the `ui_state` SQLite table.

#### Scenario: Get all UI state

- **GIVEN** the `ui_state` table contains entries `selected_project_id=proj-1` and `active_proposal_session_id=ps-abc`
- **WHEN** `GET /api/v1/ui-state` is called
- **THEN** the response is `{"selected_project_id": "proj-1", "active_proposal_session_id": "ps-abc"}`

#### Scenario: Set UI state key

- **GIVEN** the server is running
- **WHEN** `PUT /api/v1/ui-state/selected_project_id` is called with body `{"value": "proj-2"}`
- **THEN** the key is upserted in the `ui_state` table and the response is 204 No Content

#### Scenario: Delete UI state key

- **GIVEN** the `ui_state` table contains `selected_project_id=proj-1`
- **WHEN** `DELETE /api/v1/ui-state/selected_project_id` is called
- **THEN** the key is removed from the `ui_state` table and the response is 204 No Content

### Requirement: FullState UI State Inclusion

The WebSocket FullState payload SHALL include a `ui_state` field containing all stored UI preferences.

#### Scenario: FullState includes stored UI preferences

- **GIVEN** the `ui_state` table contains `selected_project_id=proj-1`
- **WHEN** the server broadcasts a FullState WebSocket message
- **THEN** the message includes `"ui_state": {"selected_project_id": "proj-1"}`

### Requirement: Dashboard Session Restoration on Reload

The dashboard frontend SHALL restore the selected project and active proposal session from the `ui_state` field in the FullState payload after a browser reload.

#### Scenario: Project selection restored

- **GIVEN** `ui_state` contains `selected_project_id=proj-1` and `proj-1` exists in the project list
- **WHEN** the dashboard receives the initial FullState message after reload
- **THEN** `proj-1` is automatically selected as the active project

#### Scenario: Proposal session restored

- **GIVEN** `ui_state` contains `active_proposal_session_id=ps-abc` and session `ps-abc` is active for the selected project
- **WHEN** the dashboard receives the proposal session list after reload
- **THEN** session `ps-abc` is automatically selected as the active proposal session tab

#### Scenario: Stale reference cleaned up

- **GIVEN** `ui_state` contains `active_proposal_session_id=ps-old` but session `ps-old` no longer exists
- **WHEN** the dashboard fetches the session list and `ps-old` is not found
- **THEN** the dashboard calls `DELETE /api/v1/ui-state/active_proposal_session_id` to clean up the stale reference
