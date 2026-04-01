## MODIFIED Requirements

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
