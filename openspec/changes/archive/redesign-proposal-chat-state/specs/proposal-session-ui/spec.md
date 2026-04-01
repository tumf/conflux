## MODIFIED Requirements

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
