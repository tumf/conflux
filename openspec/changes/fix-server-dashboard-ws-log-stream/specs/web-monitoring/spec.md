## MODIFIED Requirements

### Requirement: Dashboard UI - Real-time Updates
The web dashboard SHALL automatically update when orchestrator state changes.
The web dashboard SHALL render a fresh initial state snapshot on page load.
The web dashboard SHALL fall back to polling when WebSocket updates are unavailable.
The web dashboard SHALL display real-time log entries received via WebSocket in the Logs panel (MUST).

#### Scenario: Initial state render
- **WHEN** dashboard loads
- **THEN** JavaScript fetches the current state from `/api/state`
- **AND** the fetch disables caching (equivalent to `Cache-Control: no-store` semantics)
- **AND** dashboard renders progress and task counts from the fetched snapshot

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

#### Scenario: Polling fallback updates
- **WHEN** WebSocket is disconnected and reconnection has not succeeded
- **THEN** dashboard periodically fetches `/api/state` every 5 seconds and updates the UI
- **AND** updates continue until WebSocket is re-established

#### Scenario: Manual reload shows current state
- **WHEN** user reloads the dashboard page
- **THEN** the dashboard renders the latest orchestrator state
- **AND** the displayed progress reflects current `/api/state` content

#### Scenario: ログメッセージがリアルタイムで表示される
- **GIVEN** ダッシュボードが WebSocket を購読している
- **WHEN** サーバーが `log` タイプのメッセージを送信する
- **THEN** ダッシュボードのLogsパネルにログエントリが即座に追加される
- **AND** ログは `project_id` でフィルタリングされて表示される
