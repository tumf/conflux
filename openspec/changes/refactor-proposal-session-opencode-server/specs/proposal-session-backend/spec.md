## MODIFIED Requirements

### Requirement: proposal-session-config

The system SHALL support a `proposal_session` configuration section with fields for OpenCode Server options and session inactivity timeout. The previous ACP-specific fields (`acp_command`, `acp_args`, `acp_env`) are removed.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `opencode_command = "opencode"`, `opencode_model = null`, `opencode_agent = null`, `session_inactivity_timeout_secs = 1800`

#### Scenario: custom-config-values

**Given**: `.cflx.jsonc` contains `"proposal_session": { "opencode_command": "opencode", "opencode_model": "kani/kani/auto", "opencode_agent": "code" }`
**When**: The server parses the configuration
**Then**: The custom values are used for OpenCode Server subprocess spawning and session creation

### Requirement: proposal-session-create

The system SHALL create a proposal session by spawning an `opencode serve` process in the worktree directory and creating a session via its HTTP API.

#### Scenario: create-session-success

**Given**: A registered project with id `P1`
**When**: `POST /api/v1/projects/P1/proposal-sessions` is called
**Then**: A new worktree is created on branch `proposal/<session_id>` from project HEAD, an `opencode serve` process is started in the worktree, a session is created via `POST /session` on the OpenCode Server, and session info is returned with status 201

#### Scenario: create-multiple-sessions

**Given**: A registered project with one active proposal session
**When**: `POST /api/v1/projects/{id}/proposal-sessions` is called again
**Then**: A second independent session is created with its own worktree and OpenCode Server process

### Requirement: proposal-session-chat

The system SHALL proxy chat messages between the dashboard WebSocket and the OpenCode Server HTTP API, using SSE for real-time event streaming.

#### Scenario: send-prompt-via-proxy

**Given**: An active proposal session with an OpenCode Server running
**When**: The dashboard sends a `prompt` message via WebSocket
**Then**: The Rust server calls `POST /session/:id/prompt_async` on the OpenCode Server with the user text

#### Scenario: stream-response-via-sse

**Given**: An active proposal session with a pending prompt
**When**: The OpenCode Server emits `message.part.updated` SSE events
**Then**: The Rust server relays each text part as an `agent_message_chunk` WebSocket message to the dashboard

#### Scenario: turn-completion

**Given**: An active proposal session with a running turn
**When**: The OpenCode Server emits a `session.status` event with type `idle`
**Then**: The Rust server sends a `turn_complete` WebSocket message to the dashboard

### Requirement: proposal-session-message-restore

The system SHALL restore chat history when a proposal session is reconnected.

#### Scenario: restore-messages-on-reconnect

**Given**: A proposal session with 3 previous messages (user, assistant, user)
**When**: The dashboard reconnects to the session WebSocket
**Then**: The Rust server fetches messages from `GET /session/:id/message` on the OpenCode Server and sends them as `history` WebSocket messages before accepting new prompts
