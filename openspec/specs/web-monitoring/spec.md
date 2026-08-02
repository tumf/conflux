# web-monitoring Specification

## Purpose

Provides HTTP-based monitoring capabilities for the orchestrator, including REST API endpoints, WebSocket real-time updates, and a web dashboard UI. Enables both TUI and Web UI to maintain state parity through a unified state model and event stream architecture.
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

#### Scenario: Run mode success shuts down web monitoring
- **GIVEN** ユーザーが `cflx run --web` を実行している
- **AND** オーケストレーションが成功裏に完了する
- **WHEN** run モードが成功終了へ遷移する
- **THEN** run モードが起動したHTTPサーバーと関連バックグラウンドタスクは停止する
- **AND** プロセスは追加の外部シグナルなしで正常終了する

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

### Requirement: Static File Serving - Dashboard

The HTTP server SHALL serve the embedded API v2 operator console and its static CSS and JavaScript assets. Static delivery MUST remain available in both retained local TUI and `cflx run --web` modes and MUST NOT depend on the removed standalone dashboard build.

#### Scenario: Access operator console

**When**: A client navigates to `/`
**Then**: The server responds with HTTP 200
**And**: The body is the embedded operator-console HTML with `Content-Type: text/html`

#### Scenario: Access retained assets

**When**: A client requests `/style.css` or `/app.js`
**Then**: The server responds with HTTP 200
**And**: It returns the matching embedded CSS or JavaScript content type

#### Scenario: Missing asset

**When**: A client requests an unknown static asset path
**Then**: The server responds with HTTP 404

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

### Requirement: ダッシュボードUI - 承認ボタン
Webダッシュボードは、各変更カードに承認/承認解除ボタンを表示してはならない（SHALL NOT）。

#### Scenario: 承認ボタンが表示されない
- **WHEN** 変更カードがダッシュボードに表示される
- **THEN** 「Approve」「Unapprove」ボタンは表示されない

### Requirement: すべての状態でtasks進捗を保持する
Web state_updateは、tasks.mdの読み取りに失敗した場合にcompleted_tasks/total_tasksを0/0で上書きしてはならない（MUST NOT）。archive/resolving中でも直前の進捗が維持されなければならない（MUST）。

#### Scenario: Archive/Resolving中にprogress取得が失敗する
- **GIVEN** 変更がArchivingまたはResolving状態である
- **AND** 直前のprogressが0/0ではない
- **WHEN** state_updateの生成時にtasks.mdの読み取りが失敗し0/0となる
- **THEN** completed_tasks/total_tasksは直前の値を維持する

### Requirement: Dashboard log panel ANSI escape rendering

The console log panel SHALL render supported ANSI SGR presentation as styled, sanitized HTML instead of displaying raw escape codes. Unsupported control sequences SHALL be stripped or rendered harmlessly. HTML in log content MUST never execute.

#### Scenario: Log message with ANSI color codes is rendered with color

**Given**: A log entry contains supported ANSI foreground or background color sequences
**When**: The console renders the entry
**Then**: Styled spans represent the supported colors
**And**: Raw escape characters are not visible

#### Scenario: Log message without ANSI codes is rendered normally

**Given**: A log entry contains no ANSI sequence
**When**: The console renders the entry
**Then**: Its text is displayed without unnecessary markup

#### Scenario: Malicious HTML in log message is sanitized

**Given**: A log entry contains HTML or script tags
**When**: The console renders the entry
**Then**: No DOM injection or script execution occurs
**And**: The literal content remains inspectable

#### Scenario: ANSI bold and underline decorations are rendered

**Given**: A log entry contains supported bold or underline SGR sequences
**When**: The console renders the entry
**Then**: The corresponding text decoration is applied after sanitization

### Requirement: API v2 browser operator console

The embedded web-monitoring interface MUST use `/api/v2` as its only production data, observation, error, and mutation contract. It MUST discover capabilities, read one coherent process snapshot, display process identity, and submit only advertised typed commands. Production browser code MUST NOT call legacy `/api/*` or `/ws` routes.

#### Scenario: Console bootstraps from one process

**Given**: A cflx process serves web monitoring
**When**: A user opens the embedded console
**Then**: The browser reads `/api/v2/health`, capabilities, and state
**And**: The rendered mode, changes, totals, and process identity come from the coherent v2 response

#### Scenario: Production assets contain no legacy client route

**Given**: The packaged web assets
**When**: Their network targets are inspected
**Then**: They do not reference legacy `/api/*` resources or legacy `/ws`

### Requirement: Secure browser authentication experience

The console MUST support authenticated and unauthenticated loopback v2 deployments without teaching unsafe credential transport. When authentication is required, it MUST provide a labeled token form, send the token only in the Authorization header, and provide a disconnect action. It MUST NOT put tokens in URLs, logs, correlation IDs, or `localStorage`. A token MAY be retained in tab-scoped `sessionStorage` for reload continuity and MUST be removed on disconnect.

#### Scenario: Unauthorized bootstrap requests a token

**Given**: The v2 API requires bearer authentication
**When**: Console bootstrap receives `unauthorized`
**Then**: The console displays an accessible authentication form
**And**: It does not repeatedly request protected resources without user action

#### Scenario: Disconnect clears browser credentials

**Given**: A user authenticated in the current tab
**When**: The user disconnects
**Then**: In-memory and tab-scoped credentials are cleared
**And**: Protected data and mutation controls are no longer presented as usable

### Requirement: Resilient browser observation and freshness

The console MUST consume authenticated SSE with `fetch()` response streaming, track `instance_id` and `event_sequence`, and process events in order. A replay gap, sequence discontinuity, malformed stream, or changed process incarnation MUST cause a coherent `/api/v2/state` refresh before live observation resumes. When streaming is unavailable the console MAY poll no-store snapshots, but it MUST communicate fresh, reconnecting, stale, and disconnected states and MUST disable mutations whenever displayed state is not trusted.

#### Scenario: Replay gap recovers through snapshot

**Given**: The console has a prior event cursor
**When**: The event stream reports a replay gap
**Then**: The console refreshes `/api/v2/state`
**And**: It resumes from the returned process identity and event cursor

#### Scenario: Disconnected state prevents mutation

**Given**: Neither event streaming nor snapshot polling can confirm current state
**When**: The console becomes stale or disconnected
**Then**: The status and last successful update are visible
**And**: Mutation controls cannot submit a command

### Requirement: Revision-safe idempotent browser commands

Every console mutation MUST use `/api/v2/commands`, the latest confirmed `state_revision`, and a 1–200 character idempotency key unique to the user's intended side effect. The console MUST prevent duplicate submission while the command is pending. It MUST reuse the same request and key only when retrying an outcome-unknown transport failure. A stale-revision response MUST refresh state and require a new user decision rather than automatically executing the command against new state.

#### Scenario: Pending action cannot be double-submitted

**Given**: A command for one target is pending
**When**: The user activates the same action again
**Then**: No second command intent is created
**And**: The control communicates its pending state

#### Scenario: Stale command requires another decision

**Given**: The console submits a command with an obsolete revision
**When**: The server returns `stale_revision`
**Then**: The console refreshes current state
**And**: It does not automatically resubmit the side effect

### Requirement: Task-oriented operator information architecture

The console MUST prioritize the information needed to understand current operation and choose a safe next action. Its initial viewport MUST communicate connection freshness, process identity, current application mode, active work, attention-required conditions, and the currently valid primary action. Changes MUST be ordered or grouped as attention required, active, waiting, and completed. Details MUST be available through visible disclosures rather than gesture-only interaction.

#### Scenario: Error state exposes recovery before summary statistics

**Given**: One or more changes require operator attention
**When**: The console renders current state
**Then**: The attention condition and recovery action appear before completed-work summaries
**And**: The user does not need to open every change to discover the blocker

#### Scenario: Change details have explicit disclosure

**Given**: A change has dependencies or additional status detail
**When**: The change row is rendered
**Then**: A labeled disclosure button exposes the details
**And**: Tap, swipe, or hover is not the only way to access them

### Requirement: Accessible destructive action confirmation

Force stop, active-change stop-and-dequeue, and worktree deletion MUST require an explicit accessible confirmation before the console submits a command. Confirmation MUST name the target and consequence, use a native dialog or equivalent conforming dialog pattern, provide safe initial focus, support cancellation and Escape before submission, prevent backdrop submission and duplicate confirmation, and restore focus to the invoking control.

#### Scenario: Cancelled destructive action has no side effect

**Given**: A destructive confirmation dialog is open
**When**: The user cancels or presses Escape
**Then**: No command is submitted
**And**: Focus returns to the action that opened the dialog

#### Scenario: Confirm submits once

**Given**: A user reviewed the destructive consequence
**When**: The user confirms with keyboard or pointer input
**Then**: Exactly one typed v2 command is submitted
**And**: Further confirmation is disabled while its outcome is pending

### Requirement: WCAG 2.2 AA operator workflow

The complete console workflow MUST conform to WCAG 2.2 Level AA. It MUST provide semantic landmarks and headings, a skip link, keyboard-operable controls, visible focus, labeled forms, programmatic tab and disclosure state, accessible dialogs, deliberate live-region announcements, and status that is not communicated by color alone. Tabs MUST implement the WAI-ARIA tabs keyboard pattern. Every touch target MUST meet the WCAG 2.2 minimum, and primary actions MUST be at least 44 by 44 CSS pixels.

#### Scenario: Keyboard user completes an operator flow

**Given**: A user operates without a pointer
**When**: They authenticate, navigate views, inspect a change, invoke and cancel a confirmation, and read an error
**Then**: Every step is available in logical focus order
**And**: Focus remains visible and returns predictably after modal interaction

#### Scenario: Dynamic updates are announced without flooding

**Given**: The console receives connection, command, and orchestration updates
**When**: User-relevant status changes
**Then**: Routine changes use polite status announcements
**And**: Failed mutations use an assertive alert while repetitive event traffic is not announced individually

### Requirement: Responsive and perceivable visual system

The console MUST remain usable at 320 CSS pixels, mobile landscape, tablet, desktop, and 200 percent zoom without page-level horizontal scrolling or loss of information or actions. Long identifiers, paths, branches, logs, and errors MUST wrap, truncate with an accessible full-value affordance, or use bounded local scrolling. Normal text contrast MUST be at least 4.5:1 and component, graphical, and focus-indicator contrast at least 3:1. The CSS MUST use defined custom properties, MUST NOT use `transition: all`, and MUST respect reduced-motion and increased-contrast preferences.

#### Scenario: Narrow viewport retains all actions

**Given**: The viewport is 320 CSS pixels wide
**When**: The console displays long change, path, and error values
**Then**: The page has no horizontal overflow
**And**: All values and controls remain discoverable and operable

#### Scenario: Reduced motion preserves state feedback

**Given**: The user prefers reduced motion
**When**: Loading, connection, disclosure, or notification state changes
**Then**: Nonessential motion is removed
**And**: Text, shape, or other non-motion feedback still communicates the state

### Requirement: Actionable logs and typed errors

The console MUST provide a persistent log view and MUST render typed v2 errors with sanitized message, stable error code, correlation ID, current revision when present, and a next recovery action. Success messages MAY expire automatically; failures requiring action MUST remain available until dismissed or resolved. Log content MUST be rendered without DOM injection, and supported ANSI presentation MUST be applied only after sanitization.

#### Scenario: Command failure explains recovery

**Given**: A v2 command returns a typed failure
**When**: The console presents it
**Then**: The user sees the message, error code, correlation ID, and relevant next action
**And**: The failure does not disappear before the user can act on it

#### Scenario: Malicious log content remains text

**Given**: A log message contains HTML or script syntax
**When**: The console renders the log
**Then**: No markup or script is executed
**And**: The message remains inspectable as text

### Requirement: V2 worktree operator experience

The console MUST read v2 worktree resources, address delete and merge only by opaque `worktree_id`, and present server-provided operation eligibility and blocked reasons. It MUST NOT infer mutation safety solely from branch, path, dirty, ahead, or conflict fields. Conflict recovery MUST direct the user to the local or TUI flow when that is the advertised recovery boundary.

#### Scenario: Ineligible operation explains why

**Given**: A worktree operation is ineligible
**When**: The Worktrees view renders the resource
**Then**: The corresponding action is unavailable
**And**: The server-provided blocked reason is visible

#### Scenario: Worktree mutation uses opaque identity

**Given**: A worktree is eligible for a remote operation
**When**: The user confirms the operation
**Then**: The command target contains its opaque `worktree_id`
**And**: No path or branch is sent as mutation identity
