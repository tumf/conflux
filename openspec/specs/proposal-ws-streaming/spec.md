## Requirements

### Requirement: agent-thought-chunk-ws-message

The proposal WebSocket server MUST send agent thought content as a distinct `agent_thought_chunk` message type, separate from `agent_message_chunk`.

#### Scenario: thought chunk arrives from ACP

**Given**: An active proposal WebSocket connection and a running ACP session
**When**: The ACP subprocess emits an `AgentThoughtChunk` session update
**Then**: The server sends a WebSocket message with `{ "type": "agent_thought_chunk", "text": "<content>" }` to the client

#### Scenario: message chunk arrives from ACP

**Given**: An active proposal WebSocket connection and a running ACP session
**When**: The ACP subprocess emits an `AgentMessageChunk` session update
**Then**: The server sends a WebSocket message with `{ "type": "agent_message_chunk", "text": "<content>" }` to the client (unchanged behavior)

### Requirement: thought-chunk-history-annotation

Thought chunks recorded in proposal session message history MUST be distinguishable from regular message chunks.

#### Scenario: thought chunk recorded in history

**Given**: A proposal session receiving an `AgentThoughtChunk` event
**When**: The chunk is appended to the session's message history
**Then**: The resulting `ProposalSessionMessageRecord` has `is_thought` set to `true`

#### Scenario: message chunk recorded in history

**Given**: A proposal session receiving an `AgentMessageChunk` event
**When**: The chunk is appended to the session's message history
**Then**: The resulting `ProposalSessionMessageRecord` has `is_thought` either absent or `false`


### Requirement: proposal-ws-server-message-types

The `ProposalWsServerMessage` enum includes `agent_thought_chunk` as a valid message type in addition to the existing types.

#### Scenario: frontend receives thought chunk

**Given**: A dashboard WebSocket client connected to a proposal session
**When**: A message with `type: "agent_thought_chunk"` is received
**Then**: The `onThoughtChunk` callback is invoked with the text content


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

## Requirements

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
