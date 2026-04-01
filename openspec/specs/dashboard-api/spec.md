## Requirements

### Requirement: stats-overview-api-contract-test

The `/api/v1/stats/overview` endpoint response JSON structure must be validated by automated tests against the frontend `StatsOverview` TypeScript type to prevent runtime type mismatches.

#### Scenario: Rust API test validates response field structure

**Given**: A server with at least one recorded change event
**When**: `GET /api/v1/stats/overview` is called
**Then**: The response JSON contains `summary` (object with `success_count`, `failure_count`, `in_progress_count`, `average_duration_ms`), `recent_events` (array of objects with `project_id`, `change_id`, `operation`, `result`, `timestamp`), and `project_stats` (array of objects with `project_id`, `apply_success_rate`, `average_duration_ms`, `success_count`, `failure_count`, `in_progress_count`)

### Requirement: stats-overview-frontend-resilience-test

The `OverviewDashboard` component must render without crashing even when the API response is missing expected fields.

#### Scenario: Dashboard renders with complete StatsOverview response

**Given**: A mocked API returning a complete `StatsOverview` response
**When**: `OverviewDashboard` is rendered
**Then**: Summary cards, recent events list, and project stats are displayed without errors

#### Scenario: Dashboard renders with partial API response

**Given**: A mocked API returning a response where `recent_events` or `project_stats` is undefined
**When**: `OverviewDashboard` is rendered
**Then**: Fallback UI is shown (e.g., "No recent events", "No project stats") and no TypeError is thrown

## Requirements

### Requirement: proposal-session-messages-endpoint

The server SHALL provide a REST endpoint `GET /api/v1/projects/{id}/proposal-sessions/{session_id}/messages` that returns the persisted message history for a proposal session as a JSON array of `ProposalSessionMessageRecord` objects.

#### Scenario: get-messages-for-active-session

**Given**: An active proposal session with user and assistant messages
**When**: A GET request is made to `/api/v1/projects/{id}/proposal-sessions/{session_id}/messages`
**Then**: The response is 200 OK with a JSON array of message records in chronological order

#### Scenario: get-messages-for-nonexistent-session

**Given**: No proposal session with the given session ID exists
**When**: A GET request is made to `/api/v1/projects/{id}/proposal-sessions/{session_id}/messages`
**Then**: The response is 404 Not Found

## Requirements

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


### Requirement: Dashboard Session Restoration on Reload

The dashboard frontend SHALL restore the selected project, active proposal session, and best-effort file-browse selection from the `ui_state` field in the FullState payload after a browser reload. When a persisted change or worktree browse target is restored successfully, the dashboard SHALL also restore enough tab state to surface the restored selection in the visible UI. If a persisted browse target no longer exists, the dashboard SHALL clear only the stale browse-related state and continue loading normally.

#### Scenario: Project selection restored

- **GIVEN** `ui_state` contains `selected_project_id=proj-1` and `proj-1` exists in the project list
- **WHEN** the dashboard receives the initial FullState message after reload
- **THEN** `proj-1` is automatically selected as the active project

#### Scenario: Proposal session restored

- **GIVEN** `ui_state` contains `active_proposal_session_id=ps-abc` and session `ps-abc` is active for the selected project
- **WHEN** the dashboard receives the proposal session list after reload
- **THEN** session `ps-abc` is automatically selected as the active proposal session tab

#### Scenario: Change selection restored

- **GIVEN** `ui_state` contains a persisted file-browse context that references change `change-a` for existing project `proj-1`
- **AND** the dashboard reloads while `change-a` is still available for that project
- **WHEN** the dashboard completes its initial restoration flow
- **THEN** the file-browse context is restored to `change-a`
- **AND** the Changes center tab and Files pane are selected so the restored change is visible

#### Scenario: Worktree selection restored

- **GIVEN** `ui_state` contains a persisted file-browse context that references worktree branch `feature-x` for existing project `proj-1`
- **AND** the dashboard reloads while worktree `feature-x` still exists for that project
- **WHEN** the dashboard completes its initial restoration flow
- **THEN** the file-browse context is restored to worktree `feature-x`
- **AND** the Worktrees center tab and Files pane are selected so the restored worktree is visible

#### Scenario: Stale browse reference cleaned up

- **GIVEN** `ui_state` contains a persisted file-browse context that references a change or worktree that no longer exists for the selected project
- **WHEN** the dashboard validates the persisted browse target during reload restoration
- **THEN** the dashboard calls the UI-state delete endpoint for the stale browse-related key or keys
- **AND** dashboard startup continues with the selected project still available

#### Scenario: Stale proposal session reference cleaned up

- **GIVEN** `ui_state` contains `active_proposal_session_id=ps-old` but session `ps-old` no longer exists
- **WHEN** the dashboard fetches the session list and `ps-old` is not found
- **THEN** the dashboard calls `DELETE /api/v1/ui-state/active_proposal_session_id` to clean up the stale reference
