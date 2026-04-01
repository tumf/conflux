## Requirements

### Requirement: proposal-session-config

The system shall support a `proposal_session` configuration section with fields for ACP command, arguments, environment variables, and session inactivity timeout.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `acp_command = "opencode"`, `acp_args = ["acp"]`, `acp_env = {}`, `session_inactivity_timeout_secs = 1800`

#### Scenario: custom-config-values

**Given**: `.cflx.jsonc` contains `"proposal_session": { "acp_command": "opencode", "acp_args": ["acp", "--model", "kani/kani/auto", "--agent", "spec"] }`
**When**: The server parses the configuration
**Then**: The custom values are used for ACP subprocess spawning

### Requirement: proposal-session-create

The system shall create a proposal session with an independent worktree and ACP subprocess for a given project.

#### Scenario: create-session-success

**Given**: A registered project with id `P1`
**When**: `POST /api/v1/projects/P1/proposal-sessions` is called
**Then**: A new worktree is created on branch `proposal/<session_id>` from project HEAD, an ACP subprocess is spawned in the worktree directory, ACP initialization completes, and session info is returned with status 201

#### Scenario: create-multiple-sessions

**Given**: A registered project with one active proposal session
**When**: `POST /api/v1/projects/{id}/proposal-sessions` is called again
**Then**: A second independent session is created with its own worktree and ACP process

### Requirement: proposal-session-list

The system shall list all active proposal sessions for a project.

#### Scenario: list-sessions

**Given**: A project with 2 active proposal sessions
**When**: `GET /api/v1/projects/{id}/proposal-sessions` is called
**Then**: Both sessions are returned with their IDs, worktree paths, branches, status, and creation timestamps

### Requirement: proposal-session-close

The system shall check for uncommitted changes before closing a proposal session and require explicit force for dirty worktrees.

#### Scenario: close-clean-session

**Given**: A proposal session with a clean worktree (no uncommitted changes)
**When**: `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` is called with `force: false`
**Then**: The ACP process is stopped, the worktree is removed, and 200 is returned

#### Scenario: close-dirty-session-warning

**Given**: A proposal session with uncommitted changes in the worktree
**When**: `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` is called with `force: false`
**Then**: A 409 response is returned with status `dirty`, a warning message, and a list of uncommitted files

#### Scenario: force-close-dirty-session

**Given**: A proposal session with uncommitted changes in the worktree
**When**: `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` is called with `force: true`
**Then**: The ACP process is stopped, the worktree is removed regardless of dirty state, and 200 is returned

### Requirement: proposal-session-merge

The system shall merge a clean proposal worktree into the project base branch and clean up the session.

#### Scenario: merge-clean-worktree

**Given**: A proposal session with committed changes and no merge conflicts
**When**: `POST /api/v1/projects/{id}/proposal-sessions/{session_id}/merge` is called
**Then**: The worktree branch is merged into the project base branch, the worktree and ACP process are cleaned up, and 200 is returned

#### Scenario: merge-dirty-worktree-rejected

**Given**: A proposal session with uncommitted changes
**When**: `POST /api/v1/projects/{id}/proposal-sessions/{session_id}/merge` is called
**Then**: A 409 error is returned indicating uncommitted changes must be resolved first

### Requirement: proposal-session-websocket

The system shall provide a WebSocket endpoint that proxies ACP JSON-RPC messages between the Dashboard client and the ACP agent subprocess.

#### Scenario: prompt-response-flow

**Given**: An active proposal session with WebSocket connected
**When**: The client sends `{ "type": "prompt", "text": "Create auth spec" }`
**Then**: The server sends `session/prompt` to ACP, and streams `session/update` notifications back as typed WebSocket messages (agent_message_chunk, tool_call, tool_call_update, turn_complete)

#### Scenario: elicitation-relay

**Given**: An active proposal session with WebSocket connected
**When**: The ACP agent sends a `session/elicitation` request (form mode)
**Then**: The server forwards it to the client as `{ "type": "elicitation", ... }`, and when the client responds, the server relays it back to ACP

#### Scenario: cancel-relay

**Given**: An active proposal session with an ongoing prompt turn
**When**: The client sends `{ "type": "cancel" }`
**Then**: The server sends `session/cancel` to ACP, and the turn ends with stop_reason `cancelled`

### Requirement: proposal-session-change-detection

The system shall detect OpenSpec changes generated in the proposal worktree.

#### Scenario: detect-generated-changes

**Given**: A proposal session where the agent has created `openspec/changes/add-auth/proposal.md` in the worktree
**When**: `GET /api/v1/projects/{id}/proposal-sessions/{session_id}/changes` is called
**Then**: The response includes `{ "id": "add-auth" }` with the title parsed from proposal.md

### Requirement: proposal-session-inactivity-timeout

The system shall stop the ACP process after a configurable period of inactivity.

#### Scenario: timeout-stops-acp

**Given**: A proposal session with `session_inactivity_timeout_secs: 60` and no activity for 60 seconds
**When**: The timeout scanner runs
**Then**: The ACP process is killed and the session status is updated (worktree is preserved)


### Requirement: proposal-session-config

The system shall support a `proposal_session` configuration section with fields for ACP command, arguments, environment variables, and session inactivity timeout.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `acp_command = "opencode"`, `acp_args = ["acp"]`, `acp_env = {}`, `session_inactivity_timeout_secs = 1800`

#### Scenario: custom-config-values

**Given**: `.cflx.jsonc` contains `"proposal_session": { "acp_command": "opencode", "acp_args": ["acp", "--model", "kani/kani/auto", "--agent", "spec"] }`
**When**: The server parses the configuration
**Then**: The custom values are used for ACP subprocess spawning

#### Scenario: relative-command-resolved-via-login-shell

**Given**: `acp_command` is set to `"opencode"` (relative path) and `opencode` is installed in a user-specific directory (e.g., `~/.bun/bin`) not in the default non-login-shell PATH
**When**: `AcpClient::spawn()` is called
**Then**: The system resolves the absolute path of `opencode` via the user's login shell (`$SHELL -l -c 'which opencode'`) and uses that absolute path to spawn the subprocess

#### Scenario: absolute-command-used-directly

**Given**: `acp_command` is set to `"/usr/local/bin/opencode"` (absolute path)
**When**: `AcpClient::spawn()` is called
**Then**: The system uses the absolute path directly without running `which`

#### Scenario: resolution-failure-falls-back-to-original

**Given**: `acp_command` is set to `"nonexistent-binary"` and `which` fails to locate it
**When**: `AcpClient::spawn()` is called
**Then**: The system falls back to the original command name `"nonexistent-binary"` (spawn will fail with the standard OS error)

## Requirements

### Requirement: proposal-session-opencode-transport

The system SHALL allow proposal sessions to use an OpenCode Server transport while preserving the existing dashboard WebSocket message contract.

#### Scenario: session-create-uses-opencode-server

**Given**: A registered project with id `P1`
**When**: `POST /api/v1/projects/P1/proposal-sessions` is called
**Then**: The server creates a worktree, starts an `opencode serve` process in that worktree, creates an OpenCode session, and returns the Conflux proposal session metadata

#### Scenario: websocket-chat-via-opencode-events

**Given**: An active proposal session backed by an OpenCode Server
**When**: The user sends a prompt via WebSocket
**Then**: OpenCode event stream updates are converted into the same WebSocket message shapes used by the existing dashboard (`agent_message_chunk`, `tool_call`, `tool_call_update`, `turn_complete`, `error`)

#### Scenario: close-session-kills-server

**Given**: An active proposal session backed by an `opencode serve` process
**When**: `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` is called for a clean session
**Then**: The OpenCode Server process is terminated and the proposal worktree is removed

### Requirement: opencode-server-client

The system SHALL provide an internal Rust client for the OpenCode Server HTTP API, capable of spawning an `opencode serve` process, performing session/message CRUD, and subscribing to real-time events via SSE.

#### Scenario: spawn-and-health-check

**Given**: The `opencode` binary is available on PATH
**When**: `OpencodeServer::spawn` is called with a working directory
**Then**: An `opencode serve --port 0` process starts, the assigned URL is captured, and `GET /global/health` returns `healthy: true`

#### Scenario: create-session-and-send-prompt

**Given**: A running `OpencodeServer` instance
**When**: `create_session` is called followed by `send_prompt_async` with user text
**Then**: The session is created on the OpenCode Server and the prompt is submitted asynchronously (HTTP 204)

#### Scenario: subscribe-to-events

**Given**: A running `OpencodeServer` instance with an active session
**When**: `subscribe_events` is called
**Then**: SSE events of types `message.part.updated` and `session.status` are parsed into typed `OpencodeEvent` variants


### Requirement: proposal-session-config

The system shall support a `proposal_session` configuration section with fields for ACP command, arguments, environment variables, and session inactivity timeout.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `acp_command = "opencode"`, `acp_args = ["acp"]`, `acp_env = {}`, `session_inactivity_timeout_secs = 1800`

#### Scenario: custom-config-values

**Given**: `.cflx.jsonc` contains `"proposal_session": { "acp_command": "opencode", "acp_args": ["acp", "--model", "kani/kani/auto", "--agent", "spec"] }`
**When**: The server parses the configuration
**Then**: The custom values are used for ACP subprocess spawning

#### Scenario: relative-command-resolved-via-login-shell

**Given**: `acp_command` is set to `"opencode"` (relative path) and `opencode` is installed in a user-specific directory (e.g., `~/.bun/bin`) not in the default non-login-shell PATH
**When**: `AcpClient::spawn()` is called
**Then**: The system resolves the absolute path of `opencode` via the user's login shell (`$SHELL -l -c 'which opencode'`) and uses that absolute path to spawn the subprocess

#### Scenario: absolute-command-used-directly

**Given**: `acp_command` is set to `"/usr/local/bin/opencode"` (absolute path)
**When**: `AcpClient::spawn()` is called
**Then**: The system uses the absolute path directly without running `which`

#### Scenario: resolution-failure-falls-back-to-original

**Given**: `acp_command` is set to `"nonexistent-binary"` and `which` fails to locate it
**When**: `AcpClient::spawn()` is called
**Then**: The system falls back to the original command name `"nonexistent-binary"` (spawn will fail with the standard OS error)

### Requirement: proposal-session-create

The system shall create a proposal session with an independent worktree and ACP subprocess for a given project.

#### Scenario: create-session-success

**Given**: A registered project with id `P1`
**When**: `POST /api/v1/projects/P1/proposal-sessions` is called
**Then**: A new worktree is created on branch `proposal/<session_id>` from project HEAD, an ACP subprocess is spawned in the worktree directory with `--cwd <worktree_path>`, ACP initialization completes, and session info is returned with status 201

#### Scenario: create-multiple-sessions

**Given**: A registered project with one active proposal session
**When**: `POST /api/v1/projects/{id}/proposal-sessions` is called again
**Then**: A second independent session is created with its own worktree and ACP process

### Requirement: proposal-session-close

The system shall check for uncommitted changes before closing a proposal session and require explicit force for dirty worktrees.

#### Scenario: close-clean-session

**Given**: A proposal session with a clean worktree (no uncommitted changes)
**When**: `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` is called with `force: false`
**Then**: The ACP process is stopped, the worktree is removed, and 200 is returned

#### Scenario: close-dirty-session-warning

**Given**: A proposal session with uncommitted changes in the worktree
**When**: `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` is called with `force: false`
**Then**: A 409 response is returned with status `dirty`, a warning message, and a list of uncommitted files

#### Scenario: force-close-dirty-session

**Given**: A proposal session with uncommitted changes in the worktree
**When**: `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` is called with `force: true`
**Then**: The ACP process is stopped, the worktree is removed regardless of dirty state, and 200 is returned

### Requirement: proposal-session-merge

The system shall merge a clean proposal worktree into the project base branch and clean up the session.

#### Scenario: merge-clean-worktree

**Given**: A proposal session with committed changes and no merge conflicts
**When**: `POST /api/v1/projects/{id}/proposal-sessions/{session_id}/merge` is called
**Then**: The worktree branch is merged into the project base branch, the worktree and ACP process are cleaned up, and 200 is returned

#### Scenario: merge-dirty-worktree-rejected

**Given**: A proposal session with uncommitted changes
**When**: `POST /api/v1/projects/{id}/proposal-sessions/{session_id}/merge` is called
**Then**: A 409 error is returned indicating uncommitted changes must be resolved first

### Requirement: proposal-session-websocket

The system shall provide a WebSocket endpoint that proxies ACP JSON-RPC messages between the Dashboard client and the ACP agent subprocess.

#### Scenario: prompt-response-flow

**Given**: An active proposal session with WebSocket connected
**When**: The client sends `{ "type": "prompt", "text": "Create auth spec" }`
**Then**: The server sends `session/prompt` to ACP, and streams `session/update` notifications back as typed WebSocket messages (`agent_message_chunk`, `tool_call`, `tool_call_update`, `turn_complete`)

#### Scenario: elicitation-relay

**Given**: An active proposal session with WebSocket connected
**When**: The ACP agent sends a `session/elicitation` request (form mode)
**Then**: The server forwards it to the client as `{ "type": "elicitation", ... }`, and when the client responds, the server relays it back to ACP

#### Scenario: cancel-relay

**Given**: An active proposal session with an ongoing prompt turn
**When**: The client sends `{ "type": "cancel" }`
**Then**: The server sends `session/cancel` to ACP, and the turn ends with stop_reason `cancelled`

### Requirement: proposal-session-inactivity-timeout

The system shall stop the ACP process after a configurable period of inactivity.

#### Scenario: timeout-stops-acp

**Given**: A proposal session with `session_inactivity_timeout_secs: 60` and no activity for 60 seconds
**When**: The timeout scanner runs
**Then**: The ACP process is killed and the session status is updated (worktree is preserved)

### Requirement: proposal-session-backend-transport-single-source

The system shall use ACP as the only proposal-session chat transport in server mode.

#### Scenario: no-opencode-server-transport-remains

**Given**: The proposal-session backend is built from the repository state after this change
**When**: proposal-session transport code, tests, and canonical specs are inspected
**Then**: proposal chat depends on ACP transport only, and OpenCode Server-specific proposal-session transport code has been removed


### Requirement: proposal-session-config

The system shall support a `proposal_session` configuration section with fields for ACP command, arguments, environment variables, and session inactivity timeout. The `transport_env` field allows arbitrary environment variable overrides for the ACP subprocess.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `acp_command = "opencode"`, `acp_args = ["acp"]`, `acp_env = {}`, `session_inactivity_timeout_secs = 1800`

#### Scenario: default-spec-agent-config-auto-injected

**Given**: No `OPENCODE_CONFIG` key is present in `transport_env`
**When**: A proposal session is created
**Then**: A default `opencode-proposal.jsonc` file containing `{ "mode": "spec" }` is auto-generated in the server data directory, and `OPENCODE_CONFIG` is set to that file path in the ACP subprocess environment

#### Scenario: explicit-transport-env-takes-precedence

**Given**: `transport_env` contains `{ "OPENCODE_CONFIG": "/user/override.jsonc" }`
**When**: A proposal session is created
**Then**: The explicit `OPENCODE_CONFIG` value from `transport_env` is used, and no auto-generation occurs

### Requirement: proposal-session-create

The system shall create a proposal session with an independent worktree and ACP subprocess configured for the spec agent.

#### Scenario: create-session-uses-spec-agent

**Given**: A registered project with id `P1` and default proposal session config
**When**: `POST /api/v1/projects/P1/proposal-sessions` is called
**Then**: A new worktree is created, an ACP subprocess is spawned with `OPENCODE_CONFIG` pointing to a spec-agent config file, and the session uses the specification agent for chat

## Requirements

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


### Requirement: proposal-session-websocket

The system shall provide a WebSocket endpoint that proxies ACP or OpenCode session updates between the Dashboard client and the proposal-session backend. REST message history SHALL be the authoritative hydration baseline for initial session load/reload. WebSocket replay and reconnect recovery SHALL use stable `message_id` and `turn_id` identity on assistant/tool-call/turn-complete events so the Dashboard can reconcile updates idempotently against already hydrated state. Replay or restoration traffic SHALL NOT require the client to render the same logical user message, assistant turn, or tool-call sequence more than once.

#### Scenario: reconnect-restoration-is-idempotent

**Given**: A proposal session with persisted message history and a client that reconnects after earlier history has already been loaded
**When**: The server provides replay or restoration data for that same session
**Then**: The replay or restoration contract includes enough identity or source-boundary guarantees for the client to reconcile existing messages without duplicating them in the UI

#### Scenario: replay-identifies-existing-assistant-turn

**Given**: A proposal session with a completed assistant turn containing streamed content and tool calls
**When**: The client reconnects and the server replays that turn
**Then**: The replay data includes stable message or turn identity sufficient for the client to associate the replayed data with the existing logical assistant turn

#### Scenario: restoration-remains-session-scoped

**Given**: Two active proposal sessions exist for the same project
**When**: A client connects or reconnects to the WebSocket for one specific session
**Then**: The server sends only replay or restoration data for that targeted session and does not mix message history from the other session
