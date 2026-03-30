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
