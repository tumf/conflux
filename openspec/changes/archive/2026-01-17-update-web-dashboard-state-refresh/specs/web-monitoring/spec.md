## MODIFIED Requirements
### Requirement: REST API - Full State
The HTTP server SHALL expose complete orchestrator state via REST API.
The HTTP server SHALL prevent stale responses for `/api/state` by disabling HTTP caching.
The server SHALL return the latest change state even when updates originate from the TUI auto-refresh loop.
The server SHALL refresh the state from disk/worktree sources before responding to REST API state requests.

#### Scenario: Get full state
- **WHEN** client sends `GET /api/state`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains complete JSON state from `.opencode/orchestrator-state.json`
- **AND** response includes all changes with their progress and task details
- **AND** response has `Cache-Control: no-store` header

#### Scenario: Updated state is reflected on subsequent requests
- **WHEN** `.opencode/orchestrator-state.json` changes between two `GET /api/state` requests
- **THEN** the second response body reflects the updated state

#### Scenario: Manual reload reflects TUI refresh state
- **GIVEN** TUI の自動更新が変更一覧を更新した
- **WHEN** user reloads the dashboard page
- **THEN** the dashboard renders the latest orchestrator state
- **AND** `/api/state` には TUI の更新結果が反映されている

#### Scenario: REST API refreshes state from disk
- **GIVEN** タスク進捗が作業ツリーの tasks.md に反映されている
- **WHEN** client sends `GET /api/state`
- **THEN** server refreshes the state from disk/worktree sources before responding
- **AND** response reflects the latest progress
