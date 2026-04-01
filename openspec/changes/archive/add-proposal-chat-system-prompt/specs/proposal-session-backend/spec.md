## MODIFIED Requirements

### Requirement: proposal-session-create

The system shall create a proposal session with an independent worktree and ACP subprocess for a given project, and initialize proposal chat with dedicated specification-focused prompt guidance managed by the backend.

#### Scenario: create-session-success

**Given**: A registered project with id `P1`
**When**: `POST /api/v1/projects/P1/proposal-sessions` is called
**Then**: A new worktree is created on branch `proposal/<session_id>` from project HEAD, an ACP subprocess is spawned in the worktree directory with `--cwd <worktree_path>`, ACP initialization completes, backend-managed specification-focused prompt guidance is prepared for the session, and session info is returned with status 201

### Requirement: proposal-session-websocket

The system shall provide a WebSocket endpoint that proxies ACP JSON-RPC messages between the Dashboard client and the ACP agent subprocess, while preserving backend-managed specification-focused prompt guidance for proposal chat.

#### Scenario: prompt-response-flow

**Given**: An active proposal session with WebSocket connected
**When**: The client sends `{ "type": "prompt", "text": "Create auth spec" }`
**Then**: The server sends the prompt through ACP using the session's dedicated specification-focused guidance, and streams `session/update` notifications back as typed WebSocket messages (`agent_message_chunk`, `tool_call`, `tool_call_update`, `turn_complete`)

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
