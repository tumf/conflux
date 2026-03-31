## MODIFIED Requirements

### Requirement: proposal-ws-server-message-types

The `ProposalWsServerMessage` enum includes `agent_thought_chunk` as a valid message type in addition to the existing types. The `UserMessage` variant SHALL include an optional `client_message_id` field that echoes the client-provided ID from the corresponding `Prompt` message for optimistic update correlation.

#### Scenario: frontend receives thought chunk

**Given**: A dashboard WebSocket client connected to a proposal session
**When**: A message with `type: "agent_thought_chunk"` is received
**Then**: The `onThoughtChunk` callback is invoked with the text content

#### Scenario: user-message-includes-client-message-id

**Given**: A client sends `{ "type": "prompt", "content": "hello", "client_message_id": "user-pending-abc" }`
**When**: The server records the user message and echoes it back
**Then**: The `user_message` response includes `"client_message_id": "user-pending-abc"` alongside the server-generated `id`

#### Scenario: user-message-without-client-message-id

**Given**: A client sends `{ "type": "prompt", "content": "hello" }` without a `client_message_id` field
**When**: The server records the user message and echoes it back
**Then**: The `user_message` response omits the `client_message_id` field (backward compatible)

## ADDED Requirements

### Requirement: proposal-ws-client-message-id

The `ProposalWsClientMessage::Prompt` variant SHALL accept an optional `client_message_id` string field. When present, the server SHALL include this value in the corresponding `UserMessage` response. The field SHALL use `#[serde(default)]` for backward compatibility with clients that do not send it.

#### Scenario: prompt-with-client-message-id

**Given**: A WebSocket client sends a Prompt message with `client_message_id: "user-123"`
**When**: The server processes the prompt
**Then**: The `user_message` response includes `client_message_id: "user-123"`

#### Scenario: prompt-without-client-message-id

**Given**: A WebSocket client sends a Prompt message without a `client_message_id` field
**When**: The server processes the prompt
**Then**: The `user_message` response does not include a `client_message_id` field and processing continues normally
