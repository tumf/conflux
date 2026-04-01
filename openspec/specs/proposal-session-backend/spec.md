## Requirements

### Requirement: proposal-session-config

The system shall support a `proposal_session` configuration section with fields for ACP command, arguments, environment variables, and session inactivity timeout.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `acp_command = "opencode"`, `acp_args = ["acp"]`, `acp_env = {}`, `session_inactivity_timeout_secs = 1800`

#### Scenario: custom-config-values

**Given**: `.cflx.jsonc` contains `"proposal_session": { "acp_command": "opencode", "acp_args": ["acp", "--model", "kani/kani/auto"] }`
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

The system shall create a proposal session with an independent worktree and ACP subprocess for a given project, and initialize proposal chat with dedicated specification-focused prompt guidance managed by the backend.

#### Scenario: create-session-success

**Given**: A registered project with id `P1`
**When**: `POST /api/v1/projects/P1/proposal-sessions` is called
**Then**: A new worktree is created on branch `proposal/<session_id>` from project HEAD, an ACP subprocess is spawned in the worktree directory with `--cwd <worktree_path>`, ACP initialization completes, backend-managed specification-focused prompt guidance is prepared for the session, and session info is returned with status 201

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

The system shall provide a WebSocket endpoint that proxies ACP JSON-RPC messages between the Dashboard client and the ACP agent subprocess, while preserving backend-managed specification-focused prompt guidance for proposal chat.

#### Scenario: prompt-response-flow

**Given**: An active proposal session with WebSocket connected
**When**: The client sends `{ "type": "prompt", "text": "Create auth spec" }`
**Then**: The server sends the prompt through ACP using the session's dedicated specification-focused guidance, and streams `session/update` notifications back as typed WebSocket messages (`agent_message_chunk`, `tool_call`, `tool_call_update`, `turn_complete`)

#### Scenario: elicitation-relay

**Given**: An active proposal session with WebSocket connected
**When**: The ACP agent sends a `session/elicitation` request (form mode)
**Then**: The server forwards it to the client as `{ "type": "elicitation", ... }`, and when the client responds, the server relays it back to ACP

#### Scenario: cancel-relay

**Given**: An active proposal session with an ongoing prompt turn
**When**: The client sends `{ "type": "cancel" }`
**Then**: The server sends `session/cancel` to ACP, and the turn ends with stop_reason `cancelled`

### Requirement: proposal-session-specification-boundaries

The system shall keep proposal chat within specification-authoring boundaries through backend-managed prompt guidance rather than ACP-native agent selection, and keep that guidance as internal backend-owned runtime configuration (not external prompt-file loading dependencies).

#### Scenario: implementation-request-is-redirected

**Given**: An active proposal chat session and a user asks the assistant to implement code
**When**: The assistant responds through the proposal chat flow
**Then**: The response remains specification-focused rather than performing implementation
**And**: The response redirects the user toward proposal approval or an implementation-oriented workflow

#### Scenario: clarification-is-minimized

**Given**: An active proposal chat session and a user request is partially ambiguous
**When**: The assistant prepares its response
**Then**: The assistant prefers repository and documentation context before asking questions
**And**: The assistant asks only blocking clarifications needed to reach an implementable specification

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

### Requirement: proposal-session-backend-transport-single-source

The system shall use ACP as the only proposal-session chat transport in server mode.

#### Scenario: no-opencode-server-transport-remains

**Given**: The proposal-session backend is built from the repository state after this change
**When**: proposal-session transport code, tests, and canonical specs are inspected
**Then**: proposal chat depends on ACP transport only, and OpenCode Server-specific proposal-session transport code has been removed

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


### Requirement: proposal-session-websocket-turn-recovery

The system shall support reconnect recovery for interrupted active turns by replaying enough message/state information for the client to determine whether the interrupted turn is still active or has already completed. The endpoint shall also provide heartbeat or keepalive behavior so long-running but otherwise idle turns are less likely to be disconnected by intermediaries.

#### Scenario: reconnect-replays-completed-turn

**Given**: A proposal session WebSocket disconnects during an active turn and the server-side turn completes before the client reconnects
**When**: The client reconnects to the same proposal session
**Then**: The server replays enough history/state for the client to reconcile the turn as completed without requiring prompt resubmission

#### Scenario: reconnect-replays-in-progress-turn

**Given**: A proposal session WebSocket disconnects during an active turn and the server-side turn is still in progress when the client reconnects
**When**: The client reconnects to the same proposal session
**Then**: The server replays enough history/state for the client to reconcile the turn as still active and continue receiving updates

#### Scenario: websocket-heartbeat-during-long-turn

**Given**: An active proposal session with a long-running turn and no user-visible message chunks for an extended interval
**When**: The connection remains otherwise healthy
**Then**: The server emits heartbeat or keepalive traffic often enough to reduce idle timeout disconnect risk

#### Scenario: cancel-relay-during-recovered-turn

**Given**: An active proposal session with an ongoing prompt turn after reconnect recovery
**When**: The client sends `{ "type": "cancel" }`
**Then**: The server cancels the backing turn and the turn ends with stop_reason `cancelled`
