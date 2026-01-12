# web-monitoring Specification

## Purpose
TBD - created by archiving change add-web-monitoring. Update Purpose after archive.
## Requirements
### Requirement: HTTP Server Lifecycle
オーケストレーターは、オーケストレーション状態を監視するための任意のHTTPサーバーを提供しなければならない（SHALL）。

#### Scenario: Server enabled via CLI flag
- **WHEN** ユーザーが`--web`を指定し、CLIおよび設定ファイルでポートが未指定
- **THEN** HTTPサーバーはOSが割り当てる未使用ポート（ポート0による自動割り当て）で起動する
- **AND** 実際のバインド先（アドレス/ポート）がログに表示される
- **AND** オーケストレーターは通常通り動作を継続する

#### Scenario: Server disabled by default
- **WHEN** ユーザーが`--web`を指定せずに実行する
- **THEN** HTTPサーバーは起動しない
- **AND** ネットワークポートはバインドされない

#### Scenario: Port already in use
- **WHEN** HTTPサーバーが明示指定されたポートにバインドしようとして、そのポートが使用中
- **THEN** オーケストレーターはポート番号を含む明確なエラーメッセージを出力する
- **AND** オーケストレーターは非ゼロのステータスで終了する

#### Scenario: Graceful shutdown
- **WHEN** オーケストレーターが終了シグナル（Ctrl+C）を受信する
- **THEN** HTTPサーバーはアクティブな接続を穏やかに閉じる
- **AND** オーケストレーターは進行中のリクエスト完了を待機する
- **AND** オーケストレーターは正常に終了する

### Requirement: Configuration Options
オーケストレーターは、CLIと設定ファイルでWeb監視のパラメータを設定できなければならない（SHALL）。

#### Scenario: Port configuration via CLI
- **WHEN** ユーザーが`--web --web-port 3000`で実行する
- **THEN** HTTPサーバーはデフォルトではなくポート3000にバインドする

#### Scenario: Auto port selection by default
- **WHEN** CLIと設定ファイルの両方でポートが未指定
- **THEN** HTTPサーバーはOSが割り当てる未使用ポートで起動する
- **AND** 実際のバインド先がログに表示される

#### Scenario: Configuration via config file
- **WHEN** 設定ファイルに`web.enabled = true`と`web.port = 9000`がある
- **THEN** CLIフラグがなくてもHTTPサーバーはポート9000で起動する
- **AND** CLIで指定した値は設定ファイルより優先される

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

### Requirement: レスポンシブビューポート設定
Webダッシュボードは適切なビューポート設定により、モバイルデバイスでの表示を最適化しなければならない（SHALL）。

#### Scenario: viewport メタタグの設定
- **WHEN** ダッシュボードHTMLがロードされる
- **THEN** viewport メタタグが `width=device-width, initial-scale=1` を含む
- **AND** ページがデバイスの画面幅に合わせて表示される

#### Scenario: ピンチズーム対応
- **WHEN** ユーザーがモバイルデバイスでピンチジェスチャーを行う
- **THEN** ズームイン/アウトが可能である（`user-scalable=no` を設定しない）

### Requirement: モバイルファーストCSSレイアウト
Webダッシュボードはモバイルファーストのアプローチで、3段階のブレークポイントに対応しなければならない（SHALL）。

#### Scenario: モバイル表示（320px〜767px）
- **WHEN** 画面幅が767px以下
- **THEN** 変更リストは1カラムで縦に積み重なって表示される
- **AND** フォントサイズは最小16pxを維持する
- **AND** 進捗バーは画面幅の90%を使用する

#### Scenario: タブレット表示（768px〜1023px）
- **WHEN** 画面幅が768px以上1023px以下
- **THEN** 変更リストは2カラムグリッドで表示される
- **AND** サイドバーがある場合は折りたたみ可能になる

#### Scenario: デスクトップ表示（1024px〜）
- **WHEN** 画面幅が1024px以上
- **THEN** 変更リストは最大3カラムグリッドで表示される
- **AND** すべてのUI要素が完全に展開される

#### Scenario: 画面回転時の対応
- **WHEN** デバイスが横向きから縦向き（またはその逆）に回転する
- **THEN** レイアウトが新しい画面サイズに即座に適応する
- **AND** スクロール位置が可能な限り維持される

### Requirement: タッチフレンドリーUI
Webダッシュボードのすべてのインタラクティブ要素は、タッチ操作に適したサイズと間隔を持たなければならない（SHALL）。

#### Scenario: 最小タップターゲットサイズ
- **WHEN** ボタン、リンク、または他のインタラクティブ要素が表示される
- **THEN** タップ可能領域は最小44x44ピクセルである
- **AND** 隣接するタップターゲット間に最小8pxのスペースがある

#### Scenario: 変更リスト項目のタップ
- **WHEN** ユーザーが変更リストの項目をタップする
- **THEN** タップ領域はリスト項目全体を含む
- **AND** タップ時に視覚的フィードバック（ハイライト）が表示される

#### Scenario: タッチとマウスの両方をサポート
- **WHEN** ユーザーがタッチデバイスまたはマウスで操作する
- **THEN** 両方の入力方法で同じ機能が利用可能である
- **AND** ホバー状態はマウス使用時のみ表示される

### Requirement: タッチジェスチャー対応
Webダッシュボードは一般的なタッチジェスチャーに対応しなければならない（SHALL）。

#### Scenario: スワイプによる詳細展開
- **WHEN** ユーザーが変更リスト項目を左にスワイプする
- **THEN** その変更の詳細タスクリストが展開される
- **AND** スワイプアニメーションがスムーズに表示される

#### Scenario: スワイプによる詳細折りたたみ
- **WHEN** ユーザーが展開された詳細を右にスワイプする
- **THEN** 詳細タスクリストが折りたたまれる

#### Scenario: タップと誤操作の区別
- **WHEN** ユーザーがスワイプを開始する
- **THEN** 10px以上の水平移動があるまでタップとして扱われる
- **AND** 意図しない操作を防止する

### Requirement: モバイル向け進捗表示
Webダッシュボードの進捗表示は、モバイル画面サイズに最適化されなければならない（SHALL）。

#### Scenario: 進捗バーのレスポンシブ表示
- **WHEN** モバイル画面で変更の進捗が表示される
- **THEN** 進捗バーは画面幅に応じて適切にサイズ調整される
- **AND** パーセンテージは進捗バーの横または下に表示される

#### Scenario: タスク数の簡潔な表示
- **WHEN** モバイル画面でタスク数が表示される
- **THEN** 「5/10」のような簡潔な形式で表示される
- **AND** スペースが許せば「5/10 tasks completed」と表示される

### Requirement: 接続状態のモバイル最適化表示
WebSocket接続状態インジケーターは、モバイル画面でも視認しやすく表示されなければならない（SHALL）。

#### Scenario: 接続状態の固定表示
- **WHEN** モバイル画面でダッシュボードが表示される
- **THEN** 接続状態インジケーターは画面上部に固定される
- **AND** スクロールしても常に視認可能である

#### Scenario: 接続状態変更の通知
- **WHEN** WebSocket接続状態が変化する（接続/切断）
- **THEN** トースト通知が画面下部に3秒間表示される
- **AND** 通知は手動で閉じることも可能である

### Requirement: レスポンシブパフォーマンス
Webダッシュボードは、モバイルデバイスでも良好なパフォーマンスを維持しなければならない（SHALL）。

#### Scenario: 初期ロード時間
- **WHEN** モバイルデバイスでダッシュボードをロードする
- **THEN** First Contentful Paint が3秒以内に発生する
- **AND** Largest Contentful Paint が4秒以内に発生する

#### Scenario: インタラクション応答性
- **WHEN** ユーザーがタッチ操作を行う
- **THEN** 視覚的フィードバックが100ms以内に表示される
- **AND** アニメーションは60fpsを維持する

#### Scenario: タッチイベントの最適化
- **WHEN** 連続したタッチイベントが発生する
- **THEN** スクロールやスワイプはスロットル処理される
- **AND** 不要な再レンダリングが防止される

