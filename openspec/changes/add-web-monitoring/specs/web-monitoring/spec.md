## ADDED Requirements

### Requirement: HTTP Server Lifecycle
The orchestrator SHALL provide an optional HTTP server for web-based monitoring of orchestration state.

#### Scenario: Server enabled via CLI flag
- **WHEN** user runs orchestrator with `--web` flag
- **THEN** HTTP server starts on configured port (default 8080)
- **AND** server binds to configured address (default 127.0.0.1)
- **AND** orchestrator continues normal operation

#### Scenario: Server disabled by default
- **WHEN** user runs orchestrator without `--web` flag
- **THEN** no HTTP server is started
- **AND** no network ports are bound

#### Scenario: Port already in use
- **WHEN** HTTP server tries to bind to a port that is already in use
- **THEN** orchestrator logs clear error message with port number
- **AND** orchestrator exits with non-zero status code

#### Scenario: Graceful shutdown
- **WHEN** orchestrator receives shutdown signal (Ctrl+C)
- **THEN** HTTP server closes all active connections gracefully
- **AND** orchestrator waits for in-flight requests to complete
- **AND** orchestrator exits cleanly

### Requirement: Configuration Options
The orchestrator SHALL support configuration of web monitoring parameters via CLI and config file.

#### Scenario: Port configuration via CLI
- **WHEN** user runs with `--web --web-port 3000`
- **THEN** HTTP server binds to port 3000 instead of default

#### Scenario: Bind address configuration via CLI
- **WHEN** user runs with `--web --web-bind 0.0.0.0`
- **THEN** HTTP server accepts connections from any network interface

#### Scenario: Configuration via config file
- **WHEN** config file contains `web.enabled = true` and `web.port = 9000`
- **THEN** HTTP server starts on port 9000 even without CLI flag
- **AND** CLI flags override config file values

### Requirement: REST API - Health Check
The HTTP server SHALL provide a health check endpoint for monitoring service availability.

#### Scenario: Health check returns OK
- **WHEN** client sends `GET /api/health`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains `{"status": "ok"}`
- **AND** response has `Content-Type: application/json` header

### Requirement: REST API - Full State
The HTTP server SHALL expose complete orchestrator state via REST API.

#### Scenario: Get full state
- **WHEN** client sends `GET /api/state`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains complete JSON state from `.opencode/orchestrator-state.json`
- **AND** response includes all changes with their progress and task details

#### Scenario: State file missing
- **WHEN** client sends `GET /api/state` and state file does not exist
- **THEN** server responds with HTTP 200 status
- **AND** response body contains empty state with zero changes

### Requirement: REST API - Changes List
The HTTP server SHALL provide a summary list of all changes.

#### Scenario: Get changes list
- **WHEN** client sends `GET /api/changes`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains array of change objects
- **AND** each change includes id, completed_tasks, total_tasks, and last_modified

#### Scenario: No active changes
- **WHEN** client sends `GET /api/changes` and no changes exist
- **THEN** server responds with HTTP 200 status
- **AND** response body contains empty array `[]`

### Requirement: REST API - Single Change Detail
The HTTP server SHALL provide detailed information for a specific change.

#### Scenario: Get existing change
- **WHEN** client sends `GET /api/changes/add-web-monitoring`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains complete change object with all tasks

#### Scenario: Get non-existent change
- **WHEN** client sends `GET /api/changes/invalid-id`
- **THEN** server responds with HTTP 404 status
- **AND** response body contains error message with change ID

### Requirement: WebSocket - Real-time Updates
The HTTP server SHALL support WebSocket connections for real-time state updates.

#### Scenario: WebSocket connection established
- **WHEN** client connects to `ws://localhost:8080/ws`
- **THEN** server accepts WebSocket upgrade
- **AND** connection remains open for bidirectional communication

#### Scenario: State update broadcast
- **WHEN** orchestrator state changes (task completion, new change, etc.)
- **THEN** server broadcasts JSON message to all connected WebSocket clients
- **AND** message contains timestamp and updated change data
- **AND** message format is `{"type": "state_update", "timestamp": "...", "changes": [...]}`

#### Scenario: Multiple concurrent clients
- **WHEN** multiple clients connect via WebSocket simultaneously
- **THEN** all clients receive state update broadcasts
- **AND** each client maintains independent connection
- **AND** disconnection of one client does not affect others

#### Scenario: WebSocket client disconnection
- **WHEN** client closes WebSocket connection
- **THEN** server cleans up connection resources
- **AND** server continues broadcasting to remaining clients

### Requirement: Static File Serving - Dashboard
The HTTP server SHALL serve a web-based dashboard interface for visualizing orchestration state.

#### Scenario: Access dashboard
- **WHEN** client navigates to `http://localhost:8080/`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains HTML dashboard page
- **AND** response has `Content-Type: text/html` header

#### Scenario: Access CSS assets
- **WHEN** client requests `GET /assets/style.css`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains CSS stylesheet
- **AND** response has `Content-Type: text/css` header

#### Scenario: Access JavaScript assets
- **WHEN** client requests `GET /assets/app.js`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains JavaScript code
- **AND** response has `Content-Type: application/javascript` header

#### Scenario: Missing asset
- **WHEN** client requests non-existent asset path
- **THEN** server responds with HTTP 404 status

### Requirement: Dashboard UI - Change List Display
The web dashboard SHALL display a list of all changes with their current progress.

#### Scenario: Display changes
- **WHEN** dashboard loads and fetches change list
- **THEN** UI renders table or list of changes
- **AND** each change shows ID, completion percentage, and task count
- **AND** changes are sorted by last modified time (newest first)

#### Scenario: Empty state display
- **WHEN** no changes exist
- **THEN** dashboard displays helpful message "No changes found"

### Requirement: Dashboard UI - Real-time Updates
The web dashboard SHALL automatically update when orchestrator state changes.

#### Scenario: WebSocket connection in dashboard
- **WHEN** dashboard loads
- **THEN** JavaScript establishes WebSocket connection to `/ws`
- **AND** connection status indicator shows "Connected"

#### Scenario: Live progress updates
- **WHEN** orchestrator completes a task
- **THEN** WebSocket broadcast is received
- **AND** dashboard updates progress bar without page reload
- **AND** task count updates reflect new state

#### Scenario: WebSocket reconnection
- **WHEN** WebSocket connection is lost
- **THEN** dashboard shows "Disconnected" status
- **AND** JavaScript automatically attempts to reconnect
- **AND** reconnection succeeds when server is available

### Requirement: Dashboard UI - Task Status Visualization
The web dashboard SHALL show detailed task status for each change.

#### Scenario: Expand change details
- **WHEN** user clicks on a change in the list
- **THEN** UI expands to show all tasks for that change
- **AND** each task displays checkbox status (completed or pending)
- **AND** task descriptions are rendered from tasks.md

#### Scenario: Visual progress indication
- **WHEN** change is displayed
- **THEN** UI shows progress bar with percentage (completed_tasks / total_tasks)
- **AND** progress bar uses color coding (green for complete, yellow for in-progress)

### Requirement: Error Handling and Logging
The HTTP server SHALL handle errors gracefully and log all HTTP requests.

#### Scenario: Invalid JSON in state file
- **WHEN** `.opencode/orchestrator-state.json` contains malformed JSON
- **THEN** API endpoints return HTTP 500 status
- **AND** error is logged with details
- **AND** response body contains generic error message (not exposing internals)

#### Scenario: Request logging
- **WHEN** any HTTP request is received
- **THEN** server logs request method, path, and status code
- **AND** logs include timestamp and response time

#### Scenario: WebSocket error logging
- **WHEN** WebSocket connection encounters error
- **THEN** error is logged with connection ID
- **AND** connection is closed gracefully

### Requirement: Concurrent Access Safety
The HTTP server SHALL safely handle concurrent access to orchestrator state.

#### Scenario: Concurrent API requests
- **WHEN** multiple clients request state simultaneously
- **THEN** all requests receive consistent state snapshot
- **AND** no race conditions or data corruption occurs

#### Scenario: State updates during read
- **WHEN** API request reads state while orchestrator is updating it
- **THEN** request waits for write lock or reads previous consistent state
- **AND** no partial or corrupted data is returned

### Requirement: Performance and Scalability
The HTTP server SHALL handle reasonable load without impacting orchestration performance.

#### Scenario: Multiple WebSocket clients
- **WHEN** 10 concurrent WebSocket clients are connected
- **THEN** all clients receive updates within 100ms of state change
- **AND** orchestrator performance is not degraded

#### Scenario: Large state file
- **WHEN** state contains 100+ changes with 1000+ total tasks
- **THEN** API responses complete within 1 second
- **AND** WebSocket broadcasts complete within 200ms
