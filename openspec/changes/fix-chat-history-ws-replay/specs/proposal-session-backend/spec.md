## ADDED Requirements

### Requirement: proposal-session-ws-replay-user-messages

The server SHALL include user-role messages in the WebSocket history replay when a client connects to a proposal session WebSocket endpoint.

#### Scenario: ws-replay-includes-user-messages

**Given**: A proposal session with 2 user messages and 2 assistant messages in `message_history`
**When**: A WebSocket client connects to `/api/v1/proposal-sessions/{session_id}/ws`
**Then**: The server replays all 4 messages in chronological order, sending user messages as `user_message` events and assistant messages as existing `agent_message_chunk` + `turn_complete` events

#### Scenario: user-message-event-format

**Given**: A user message with id `session-1-user-1`, content `Hello`, timestamp `2026-03-30T00:00:00Z` in session history
**When**: The WebSocket replay sends this message
**Then**: The event is `{ "type": "user_message", "id": "session-1-user-1", "content": "Hello", "timestamp": "2026-03-30T00:00:00Z" }`

## MODIFIED Requirements

### Requirement: proposal-session-ui-history-hydration

The Dashboard SHALL restore existing proposal-session messages when reconnecting to or reopening the same session, using WebSocket replay as the sole history restoration mechanism.

#### Scenario: reopen-session-restores-history-via-ws

**Given**: A proposal session with existing user and assistant messages persisted by the backend
**When**: The user closes and reopens the chat for that same session
**Then**: The WebSocket connection replays all prior messages (user messages as `user_message` events, assistant messages as streaming events), and the chat list is populated before any new streaming updates are rendered

#### Scenario: no-rest-hydrate-on-mount

**Given**: A proposal session chat view is mounted
**When**: The component initializes
**Then**: No REST API call to `listProposalSessionMessages` is made; history restoration relies entirely on WebSocket replay
