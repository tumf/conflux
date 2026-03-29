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
