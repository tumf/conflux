## ADDED Requirements

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

## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: REST API - Health Check

Superseded by the versioned remote-control health resource.

#### Scenario: Legacy health is absent

**When**: A client requests `/api/health`
**Then**: The server returns not found

### Requirement: REST API - Full State

Superseded by coherent `/api/v2/state` projection.

#### Scenario: Legacy state is absent

**When**: A client requests `/api/state`
**Then**: The server returns not found

### Requirement: REST API - Changes List

Superseded by `/api/v2/changes`.

#### Scenario: Legacy changes list is absent

**When**: A client requests `/api/changes`
**Then**: The server returns not found

### Requirement: REST API - Single Change Detail

Superseded by the v2 change resource.

#### Scenario: Legacy change detail is absent

**When**: A client requests a legacy change-detail route
**Then**: The server returns not found

### Requirement: WebSocket - Real-time Updates

Superseded for browsers by authenticated fetch-streamed v2 SSE.

#### Scenario: Legacy browser WebSocket is absent

**When**: A browser connects to `/ws`
**Then**: The server returns not found

### Requirement: Dashboard UI - Change List Display

Superseded by the task-oriented operator information architecture.

#### Scenario: New change presentation applies

**When**: The console renders changes
**Then**: It uses the API v2 operator-console requirements

### Requirement: Dashboard UI - Real-time Updates

Superseded by resilient browser observation and revision-safe commands.

#### Scenario: New observation contract applies

**When**: The console observes state
**Then**: It uses v2 SSE, revision, and recovery behavior

### Requirement: Dashboard UI - Task Status Visualization

Superseded by coherent v2 display status and accessible change presentation.

#### Scenario: V2 status presentation applies

**When**: A change is rendered
**Then**: Its display status comes from the v2 projection

### Requirement: タッチジェスチャー対応

Gesture-only disclosure is removed in favor of explicit controls usable by every input method.

#### Scenario: Gesture is optional

**When**: A user has no touch gesture input
**Then**: Every detail and action remains available

### Requirement: 接続状態のモバイル最適化表示

Superseded by responsive freshness and connection-state requirements.

#### Scenario: Freshness is available at every viewport

**When**: Connection state changes
**Then**: The console communicates freshness without a mobile-only contract

### Requirement: REST API - 変更の承認

Obsolete legacy-route prohibition after the legacy router is removed.

#### Scenario: Legacy approval route is absent

**When**: A client requests the legacy approval route
**Then**: The server returns not found

### Requirement: REST API - 変更の承認解除

Obsolete legacy-route prohibition after the legacy router is removed.

#### Scenario: Legacy unapproval route is absent

**When**: A client requests the legacy unapproval route
**Then**: The server returns not found

### Requirement: 承認状態変更のWebSocket通知

Obsolete with legacy WebSocket removal.

#### Scenario: No legacy approval event

**When**: State changes
**Then**: No legacy approval WebSocket message is emitted

### Requirement: Web Dashboard Execution Controls

Superseded by typed revision-safe v2 commands and destructive-action confirmation.

#### Scenario: V2 controls apply

**When**: A user invokes an execution action
**Then**: The console uses the v2 command safety contract

### Requirement: Execution Control API

Superseded by `/api/v2/commands`.

#### Scenario: Legacy control routes are absent

**When**: A client posts to a legacy control route
**Then**: The server returns not found
**And**: No side effect occurs

### Requirement: Web App Mode Vocabulary

Superseded by `InstanceSnapshot.app_mode` in the v2 contract.

#### Scenario: V2 mode is displayed

**When**: The console renders current mode
**Then**: It uses the coherent v2 snapshot value

### Requirement: REST API - Worktrees List

Superseded by versioned remote worktree resources.

#### Scenario: Legacy worktree list is absent

**When**: A client requests `/api/worktrees`
**Then**: The server returns not found

### Requirement: REST API - Worktree Operations

Superseded by closed v2 worktree commands.

#### Scenario: Legacy worktree mutations are absent

**When**: A client posts to a legacy worktree operation route
**Then**: The server returns not found
**And**: No Git or process side effect occurs

### Requirement: WebSocket - Worktree Parity Updates

Superseded by v2 worktree reads, projection events, and snapshot recovery.

#### Scenario: No legacy worktree snapshot event

**When**: Worktree state changes
**Then**: The console recovers through versioned resources

### Requirement: Dashboard UI - Worktrees View

Superseded by the v2 worktree operator experience.

#### Scenario: V2 worktree view applies

**When**: The console renders worktrees
**Then**: Opaque identity and server-provided eligibility govern operations

### Requirement: Worktree Operations Logging and Failure Policy

Superseded by stable typed v2 errors, correlation, and shared worktree service behavior.

#### Scenario: V2 worktree failure applies

**When**: A worktree command fails
**Then**: The console receives and presents the typed v2 failure

### Requirement: Web ステータスは Reducer から導出される

Superseded by the versioned projection-owner contract.

#### Scenario: V2 projection owns display status

**When**: The console reads a change
**Then**: Its status is the v2 reducer-derived display status

### Requirement: Control API State Transitions

Superseded by closed shared command delegation and server-side lifecycle validation.

#### Scenario: Invalid v2 transition has no side effect

**When**: A command is invalid for current lifecycle state
**Then**: The shared command service rejects it
**And**: The console presents the typed failure
