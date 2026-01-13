## MODIFIED Requirements

### Requirement: REST API - Full State
The HTTP server SHALL expose complete orchestrator state via REST API.
The HTTP server SHALL prevent stale responses for `/api/state` by disabling HTTP caching.

#### Scenario: Get full state
- **WHEN** client sends `GET /api/state`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains complete JSON state from `.opencode/orchestrator-state.json`
- **AND** response includes all changes with their progress and task details
- **AND** response has `Cache-Control: no-store` header

#### Scenario: State file missing
- **WHEN** client sends `GET /api/state` and state file does not exist
- **THEN** server responds with HTTP 200 status
- **AND** response body contains empty state with zero changes
- **AND** response has `Cache-Control: no-store` header

#### Scenario: Updated state is reflected on subsequent requests
- **WHEN** `.opencode/orchestrator-state.json` changes between two `GET /api/state` requests
- **THEN** the second response body reflects the updated state

### Requirement: WebSocket - Real-time Updates
HTTPサーバーは、WebSocket接続を通じてリアルタイムの状態更新をサポートしなければならない（SHALL）。
WebSocket接続確立時、サーバーはクライアントに最新の状態スナップショットを送信しなければならない（SHALL）。

**変更内容**: TUIモードでも状態更新がブロードキャストされるよう、TUIオーケストレーターがWebStateへの参照を持ち、状態変更時にupdate()を呼び出すことを明確化。

#### Scenario: WebSocket connection established
- **WHEN** クライアントが`ws://localhost:8080/ws`に接続する
- **THEN** サーバーはWebSocketアップグレードを受け入れる
- **AND** 双方向通信のために接続が維持される
- **AND** サーバーは接続直後に最新状態を`state_update`メッセージとして送信する

#### Scenario: State update broadcast
- **WHEN** オーケストレーターの状態が変化する（タスク完了、新しい変更など）
- **THEN** サーバーはすべての接続されたWebSocketクライアントにJSONメッセージをブロードキャストする
- **AND** メッセージにはタイムスタンプと更新された変更データが含まれる
- **AND** メッセージ形式は`{"type": "state_update", "timestamp": "...", "changes": [...]}`である

#### Scenario: TUI mode state updates
- **WHEN** TUIモードで`--web`オプションを使用して起動する
- **THEN** WebStateがTUIオーケストレーターに渡される
- **AND** オーケストレーターのループ内で状態変更時にWebStateのupdate()が呼び出される
- **AND** WebSocketクライアントに状態更新がブロードキャストされる

#### Scenario: Multiple concurrent clients
- **WHEN** 複数のクライアントが同時にWebSocket経由で接続する
- **THEN** すべてのクライアントが状態更新ブロードキャストを受信する
- **AND** 各クライアントは独立した接続を維持する
- **AND** 1つのクライアントの切断は他のクライアントに影響しない

#### Scenario: WebSocket client disconnection
- **WHEN** クライアントがWebSocket接続を閉じる
- **THEN** サーバーは接続リソースをクリーンアップする
- **AND** サーバーは残りのクライアントへのブロードキャストを継続する

### Requirement: Dashboard UI - Real-time Updates
The web dashboard SHALL automatically update when orchestrator state changes.
The web dashboard SHALL render a fresh initial state snapshot on page load.
The web dashboard SHALL fall back to polling when WebSocket updates are unavailable.

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
