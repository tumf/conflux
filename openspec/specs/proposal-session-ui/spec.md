## Requirements

### Requirement: proposal-session-ui-start

The Dashboard shall provide a button to start a new proposal session from the project detail view.

#### Scenario: start-session-from-button

**Given**: A project detail view is displayed in the Dashboard
**When**: The user clicks "Add Proposal"
**Then**: A new proposal session is created via the API, and the chat interface is displayed

### Requirement: proposal-session-ui-chat

The Dashboard shall provide a chat interface for conversing with the ACP agent during a proposal session. Messages SHALL render Markdown including headings (h1-h3), unordered/ordered lists, links, horizontal rules, code blocks (with language labels and copy buttons), bold text, and inline code. Assistant messages SHALL show a copy button on hover. All messages SHALL display a relative timestamp on hover.

#### Scenario: send-and-receive-messages

**Given**: An active proposal session chat view
**When**: The user types a message and submits
**Then**: The message is sent via WebSocket, and agent responses stream into the message list with Markdown rendering

#### Scenario: tool-call-display

**Given**: The agent executes a tool during a prompt turn
**When**: Tool call updates arrive via WebSocket
**Then**: The tool call is displayed inline with its title and status (pending → in_progress → completed)

#### Scenario: render-markdown-headings-and-lists

**Given**: An assistant message containing Markdown headings and lists
**When**: The message is displayed
**Then**: Headings are rendered as styled h1-h3 elements, and lists are rendered with proper bullets/numbers and indentation

#### Scenario: render-markdown-links

**Given**: An assistant message containing `[text](url)` links
**When**: The message is displayed
**Then**: Links are rendered as clickable `<a>` elements that open in a new tab

#### Scenario: code-block-copy-button

**Given**: An assistant message containing a fenced code block
**When**: The message is displayed
**Then**: The code block shows a language label (if specified) and a copy button that copies the code content to clipboard

#### Scenario: message-copy-button

**Given**: An assistant message is displayed
**When**: The user hovers over the message
**Then**: A copy button appears that copies the full message content to clipboard when clicked

#### Scenario: message-timestamp-on-hover

**Given**: Any message is displayed
**When**: The user hovers over the message
**Then**: A relative timestamp (e.g., "2 min ago") is shown

### Requirement: proposal-session-ui-elicitation

The Dashboard shall render ACP form-mode elicitation requests as interactive UI forms.

#### Scenario: render-enum-selection

**Given**: An elicitation request with a string property using `oneOf` enum values
**When**: The elicitation is displayed
**Then**: A select/radio input is rendered with the enum options and the user can choose one

#### Scenario: submit-elicitation-response

**Given**: An elicitation form is displayed
**When**: The user fills in the form and clicks submit
**Then**: An `accept` response with the form data is sent via WebSocket

#### Scenario: cancel-elicitation

**Given**: An elicitation form is displayed
**When**: The user dismisses the dialog
**Then**: A `cancel` response is sent via WebSocket

### Requirement: proposal-session-ui-changes

The Dashboard shall display OpenSpec changes detected in the proposal worktree.

#### Scenario: list-detected-changes

**Given**: The agent has generated `openspec/changes/add-auth/proposal.md` in the worktree
**When**: The changes sidebar is displayed
**Then**: The change `add-auth` appears in the list with its title

### Requirement: proposal-session-ui-close

The Dashboard shall warn users about uncommitted changes when closing a proposal session.

#### Scenario: close-dirty-session-warning

**Given**: A proposal session with uncommitted changes
**When**: The user clicks the close button
**Then**: A confirmation dialog is shown listing uncommitted files with a "Force Close" button

#### Scenario: force-close-confirmation

**Given**: The dirty session confirmation dialog is displayed
**When**: The user clicks "Force Close"
**Then**: The session is closed via the API with `force: true`

### Requirement: proposal-session-ui-merge

The Dashboard shall provide a merge button to merge the proposal worktree into the base branch.

#### Scenario: merge-clean-session

**Given**: A proposal session with committed changes and clean worktree
**When**: The user clicks "Merge"
**Then**: The session is merged via the API, the session is closed, and the user returns to the project view

### Requirement: proposal-session-ui-multi-session

The Dashboard shall support multiple simultaneous proposal sessions per project with tab-based switching.

#### Scenario: switch-between-sessions

**Given**: Two active proposal sessions for the same project
**When**: The user clicks on the second session tab
**Then**: The chat view switches to the second session's conversation

### Requirement: proposal-session-ui-stable-turn-identity

The Dashboard SHALL preserve each completed assistant turn as a separate message in a proposal session.

#### Scenario: two-sequential-turns

**Given**: A proposal session with one completed assistant response
**When**: The user sends a second prompt and a second assistant response completes
**Then**: Both assistant responses remain visible as distinct messages in chronological order

### Requirement: proposal-session-ui-history-hydration

The Dashboard SHALL restore existing proposal-session messages when reconnecting to or reopening the same session.

#### Scenario: reopen-session-restores-history

**Given**: A proposal session with existing user and assistant messages persisted by the backend
**When**: The user closes and reopens the chat for that same session
**Then**: The prior messages are loaded into the chat list before any new streaming updates are rendered

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model active-turn state explicitly and SHALL enable or disable input based on that state. When the WebSocket connection closes unexpectedly during an active turn, the turn SHALL be treated as failed and the input SHALL be re-enabled.

#### Scenario: input-disabled-only-during-active-turn

**Given**: An active proposal session and a connected WebSocket
**When**: A prompt is submitted and no completion/error/cancel event has yet been received
**Then**: The input is disabled

#### Scenario: input-reenabled-on-turn-finish

**Given**: An active proposal session with disabled input because a turn is running
**When**: The current turn completes, errors, or is cancelled
**Then**: The input is enabled unless an elicitation dialog remains active or the connection is disconnected

#### Scenario: input-reenabled-on-ws-disconnect-during-turn

**Given**: An active proposal session with disabled input because a turn is running
**When**: The WebSocket connection closes unexpectedly
**Then**: The active turn is treated as failed, `isAgentResponding` is set to false, and the input is re-enabled (though still disabled due to disconnected state, it will become usable upon reconnection)

### Requirement: proposal-session-ui-semantic-tokens

The Dashboard chat components SHALL use semantic color tokens defined in the CSS `@theme` block rather than hardcoded hex color values. This ensures consistency and enables future theming.

#### Scenario: no-hardcoded-hex-in-chat-components

**Given**: The chat-related component source files (ProposalChat, ChatMessageList, ChatInput, ToolCallIndicator, ProposalChangesList, ElicitationDialog)
**When**: The source code is inspected
**Then**: No hardcoded hex color values (e.g., `#27272a`, `#6366f1`) are used in Tailwind class names; all colors reference semantic tokens

### Requirement: proposal-session-ui-mobile-changes-drawer

The Dashboard SHALL provide access to the proposal session Changes list on mobile viewports (below the `md` breakpoint) via a slide-in drawer accessible from the chat header.

#### Scenario: open-changes-drawer-on-mobile

**Given**: A proposal session chat view on a viewport narrower than 768px
**When**: The user taps the Changes toggle button in the chat header
**Then**: A drawer slides in from the right showing the ProposalChangesList with a backdrop overlay

#### Scenario: close-drawer-on-backdrop-tap

**Given**: The Changes drawer is open on mobile
**When**: The user taps the backdrop area outside the drawer
**Then**: The drawer closes

#### Scenario: close-drawer-on-escape

**Given**: The Changes drawer is open on mobile
**When**: The user presses the Escape key
**Then**: The drawer closes

#### Scenario: drawer-hidden-on-desktop

**Given**: A proposal session chat view on a viewport 768px or wider
**When**: The chat view is displayed
**Then**: The Changes toggle button is not visible and the sidebar renders inline as before

### Requirement: proposal-session-ui-smart-scroll

The Dashboard chat message list SHALL auto-scroll to the bottom only when the user is already near the bottom of the scroll area (within 100px). When the user has scrolled up and new content arrives, a "New messages" indicator button SHALL appear. Clicking the indicator SHALL scroll to the bottom.

#### Scenario: no-forced-scroll-when-reading-history

**Given**: The user has scrolled up more than 100px from the bottom in an active chat
**When**: A new streaming chunk or message arrives
**Then**: The scroll position remains unchanged and a "New messages" pill button appears at the bottom of the viewport

#### Scenario: click-new-messages-scrolls-to-bottom

**Given**: The "New messages" pill button is visible
**When**: The user clicks it
**Then**: The chat scrolls to the bottom and the pill disappears

### Requirement: proposal-session-ui-enter-to-send

The Dashboard chat input SHALL send the message when the user presses Enter (without modifier keys). Shift+Enter SHALL insert a newline.

#### Scenario: enter-sends-message

**Given**: The chat input has text and is enabled
**When**: The user presses Enter
**Then**: The message is sent and the input is cleared

#### Scenario: shift-enter-inserts-newline

**Given**: The chat input has text and is enabled
**When**: The user presses Shift+Enter
**Then**: A newline character is inserted at the cursor position without sending

### Requirement: proposal-session-ui-typing-indicator

The Dashboard SHALL display a typing indicator (animated dots) when the agent is responding but no streaming content has been received yet.

#### Scenario: show-typing-indicator-before-stream

**Given**: A prompt has been submitted and `isAgentResponding` is true
**When**: No streaming content or tool calls have been received for the current turn
**Then**: An animated typing indicator with "Agent is thinking..." text is displayed below the last message

#### Scenario: hide-typing-indicator-on-stream-start

**Given**: The typing indicator is visible
**When**: The first streaming chunk or tool call arrives
**Then**: The typing indicator is hidden and replaced by the streaming content

### Requirement: proposal-session-ui-empty-state

The Dashboard SHALL display an informative empty state when a proposal session has no messages, including an icon, description, and example prompts.

#### Scenario: new-session-empty-state

**Given**: A newly created proposal session with no messages
**When**: The chat view is displayed
**Then**: A Bot icon, descriptive text ("Start a conversation..."), and at least 2 clickable example prompts are shown

## Requirements

### Requirement: proposal-session-ui-send-retry

The Dashboard SHALL queue user messages sent while the WebSocket is disconnected and automatically send them upon reconnection. Failed sends SHALL be visually indicated with a retry mechanism.

#### Scenario: queue-message-while-disconnected

**Given**: A proposal session with a disconnected WebSocket
**When**: The user submits a message
**Then**: The message appears in the chat list with a "pending" visual indicator and is queued for sending

#### Scenario: auto-send-on-reconnection

**Given**: One or more pending messages are queued
**When**: The WebSocket reconnects
**Then**: The queued messages are sent automatically in chronological order and their status transitions to "sent"

#### Scenario: show-failed-with-retry

**Given**: A pending message that failed to send after reconnection
**When**: The send attempt results in an error
**Then**: The message displays a "failed" visual state with a "Retry" button

#### Scenario: retry-sends-message

**Given**: A failed message with a visible "Retry" button
**When**: The user clicks "Retry"
**Then**: The message status transitions to "pending" and a new send attempt is made


### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model chat turn lifecycle as a single status state machine with values `ready`, `submitted`, `streaming`, and `error`. The status SHALL be managed by a single `useProposalChat` hook that encapsulates WebSocket connection, message state, and turn tracking. The textarea input SHALL always remain editable; only the send button and submission SHALL be gated by `status === 'ready'`. When the WebSocket connection closes unexpectedly during an active turn, the status SHALL transition to `error` and the send button SHALL remain disabled until reconnection succeeds.

#### Scenario: status-transitions-on-send

**Given**: An active proposal session with status `ready`
**When**: The user submits a message
**Then**: The status transitions to `submitted`; the send button is disabled; the textarea remains editable

#### Scenario: status-transitions-to-streaming

**Given**: Status is `submitted` after sending a prompt
**When**: The first `agent_message_chunk` or `tool_call` arrives
**Then**: The status transitions to `streaming`

#### Scenario: status-returns-to-ready

**Given**: Status is `streaming` during an active turn
**When**: A `turn_complete` message is received
**Then**: The status transitions to `ready` and the send button is re-enabled

#### Scenario: status-transitions-to-error-on-disconnect

**Given**: Status is `submitted` or `streaming` during an active turn
**When**: The WebSocket connection closes unexpectedly
**Then**: The status transitions to `error`, the active turn is treated as failed, and automatic reconnection begins

#### Scenario: textarea-always-editable

**Given**: Any chat status (`ready`, `submitted`, `streaming`, or `error`)
**When**: The user attempts to type in the textarea
**Then**: The textarea accepts input regardless of status

### Requirement: proposal-session-ui-single-message-source

The Dashboard SHALL maintain all chat messages (user and assistant, including in-progress streaming) in a single `messages[]` array managed by the `useProposalChat` hook. There SHALL be no separate `streamingContent` or `activeTurnBySessionId` store. During streaming, the assistant's in-progress message SHALL be updated in-place as the last entry in the array.

#### Scenario: streaming-content-in-messages-array

**Given**: An active proposal session receiving agent message chunks
**When**: Streaming chunks arrive
**Then**: The content is appended to the last message in the `messages[]` array (role: assistant) rather than stored in a separate streaming content map

#### Scenario: no-separate-streaming-store

**Given**: The `useAppStore` state
**When**: A proposal chat is active
**Then**: The store does not contain `streamingContent`, `activeTurnBySessionId`, or `isAgentResponding` fields

### Requirement: proposal-session-ui-client-message-id

The Dashboard SHALL include a `client_message_id` when sending prompts via WebSocket and SHALL use the server's echoed `client_message_id` in the `user_message` response to correlate optimistic messages. This prevents duplicate messages on rapid submission.

#### Scenario: optimistic-message-replaced-by-server-confirmation

**Given**: The user submits a message creating an optimistic entry with `client_message_id: "user-pending-abc"`
**When**: The server responds with `{ "type": "user_message", "id": "srv-123", "client_message_id": "user-pending-abc", ... }`
**Then**: The optimistic message is replaced (not duplicated) with the server-confirmed message using server ID

#### Scenario: no-duplicate-on-rapid-enter

**Given**: An active proposal session with status `ready`
**When**: The user presses Enter twice rapidly
**Then**: Only one message is created because the first send transitions status to `submitted`, blocking the second

### Requirement: proposal-session-ui-ws-reconnection

The proposal session WebSocket SHALL automatically reconnect with exponential backoff when disconnected. On reconnection, pending messages SHALL be flushed and message history SHALL be replayed from the server.

#### Scenario: auto-reconnect-on-disconnect

**Given**: An active proposal session WebSocket that disconnects
**When**: The disconnection is detected
**Then**: Reconnection attempts begin with exponential backoff (1s, 2s, 4s, 8s, 16s, max 30s) up to 10 retries

#### Scenario: flush-pending-on-reconnect

**Given**: Messages were queued while disconnected
**When**: The WebSocket reconnects successfully
**Then**: Queued messages are sent automatically and their status transitions from "pending" to "sent"

#### Scenario: queued-badge-clears-on-confirmation

**Given**: A message displaying "Queued (will send on reconnect)"
**When**: The server confirms receipt via `user_message` with matching `client_message_id`
**Then**: The queued indicator is removed and the message displays normally

### Requirement: proposal-session-ui-send-retry

The Dashboard SHALL queue user messages sent while the WebSocket is disconnected and automatically send them upon reconnection. Failed sends SHALL be visually indicated with a retry mechanism.

#### Scenario: queue-message-while-disconnected

**Given**: A proposal session with a disconnected WebSocket
**When**: The user submits a message
**Then**: The message appears in the chat list with a "pending" visual indicator and is queued for sending

#### Scenario: auto-send-on-reconnection

**Given**: One or more pending messages are queued
**When**: The WebSocket reconnects
**Then**: The queued messages are sent automatically in chronological order and their status transitions to "sent"

#### Scenario: show-failed-with-retry

**Given**: A pending message that failed to send after reconnection
**When**: The send attempt results in an error
**Then**: The message displays a "failed" visual state with a "Retry" button

#### Scenario: retry-sends-message

**Given**: A failed message with a visible "Retry" button
**When**: The user clicks "Retry"
**Then**: The message status transitions to "pending" and a new send attempt is made

### Requirement: proposal-session-ui-hook-encapsulation

The `ProposalChat` component SHALL receive at most 7 props (excluding `children`): `projectId`, `sessionId`, `onBack`, `onMerge`, `onClose`, `onClickChange`, and `isLoading`. All chat state management, WebSocket communication, and turn lifecycle SHALL be encapsulated within the `useProposalChat` hook used internally by `ProposalChat`.

#### Scenario: minimal-props-interface

**Given**: The `ProposalChat` component definition
**When**: Its props interface is inspected
**Then**: It accepts at most 7 props: `projectId`, `sessionId`, `onBack`, `onMerge`, `onClose`, `onClickChange`, and `isLoading`


### Requirement: proposal-session-ui-typing-indicator

The Dashboard SHALL display a typing indicator (animated dots) when the agent is responding but no assistant message content has been received yet. The indicator SHALL be hidden as soon as an assistant-role message appears in the `messages[]` array (via `agent_message_chunk` or `tool_call`). The indicator SHALL NOT depend on a separate `streamingContent` map.

#### Scenario: show-typing-indicator-before-stream

**Given**: A prompt has been submitted and the chat status is `submitted`
**When**: No assistant message exists as the last entry in the messages array
**Then**: An animated typing indicator with "Agent is thinking..." text is displayed below the last message

#### Scenario: hide-typing-indicator-on-stream-start

**Given**: The typing indicator is visible
**When**: The first `agent_message_chunk` arrives and an assistant message is appended to the messages array
**Then**: The typing indicator is hidden

#### Scenario: hide-typing-indicator-on-tool-call

**Given**: The typing indicator is visible and status is `submitted`
**When**: A `tool_call` event arrives (agent uses tools without emitting text chunks)
**Then**: The status transitions to `streaming` and the typing indicator is hidden once the assistant message with tool calls is present in the array

### Requirement: proposal-session-ui-history-hydration

The Dashboard SHALL restore existing proposal-session messages when reconnecting to or reopening the same session. Messages SHALL be loaded via a REST API endpoint before WebSocket connection is established to avoid race conditions.

#### Scenario: reopen-session-restores-history

**Given**: A proposal session with existing user and assistant messages persisted by the backend
**When**: The user closes and reopens the chat for that same session
**Then**: The prior messages are loaded from the REST endpoint into the chat list before the WebSocket connects

#### Scenario: browser-reload-restores-all-messages

**Given**: A proposal session with both user and assistant messages
**When**: The user reloads the browser
**Then**: Both user and assistant messages are restored from the REST endpoint

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model chat turn lifecycle as a single status state machine with values `ready`, `submitted`, `streaming`, and `error`. The status SHALL transition to `streaming` on receipt of either `agent_message_chunk` or `tool_call` events. The textarea input SHALL always remain editable; only the send button and submission SHALL be gated by `status === 'ready'`. When the WebSocket connection closes unexpectedly during an active turn, the status SHALL transition to `error`.

#### Scenario: status-transitions-on-send

**Given**: An active proposal session with status `ready`
**When**: The user submits a message
**Then**: The status transitions to `submitted`; the send button is disabled; the textarea remains editable

#### Scenario: status-transitions-to-streaming-on-chunk

**Given**: Status is `submitted` after sending a prompt
**When**: The first `agent_message_chunk` arrives
**Then**: The status transitions to `streaming`

#### Scenario: status-transitions-to-streaming-on-tool-call

**Given**: Status is `submitted` after sending a prompt
**When**: A `tool_call` event arrives before any `agent_message_chunk`
**Then**: The status transitions to `streaming`

#### Scenario: status-returns-to-ready

**Given**: Status is `streaming` during an active turn
**When**: A `turn_complete` message is received
**Then**: The status transitions to `ready` and the send button is re-enabled

#### Scenario: status-transitions-to-error-on-disconnect

**Given**: Status is `submitted` or `streaming` during an active turn
**When**: The WebSocket connection closes unexpectedly
**Then**: The status transitions to `error`

#### Scenario: textarea-always-editable

**Given**: Any chat status (`ready`, `submitted`, `streaming`, or `error`)
**When**: The user attempts to type in the textarea
**Then**: The textarea accepts input regardless of status
