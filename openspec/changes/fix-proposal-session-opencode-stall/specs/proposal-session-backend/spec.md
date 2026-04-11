## MODIFIED Requirements

### Requirement: proposal-session-websocket

The system shall provide a WebSocket endpoint that proxies ACP JSON-RPC messages between the Dashboard client and the ACP agent subprocess, while preserving backend-managed specification-focused prompt guidance for proposal chat. WebSocket-scoped relay tasks MUST terminate with the WebSocket connection so stale tasks cannot continue consuming ACP notifications after disconnect.

#### Scenario: reconnect-does-not-lose-session-updates

**Given**: An active proposal session whose first WebSocket connection disconnects while the ACP subprocess remains alive
**When**: A new WebSocket client reconnects to the same proposal session and the ACP subprocess emits subsequent `session/update` notifications
**Then**: The server delivers those updates to the new connection
**And**: No stale relay task from the disconnected socket continues consuming notifications for that session

#### Scenario: websocket-exit-aborts-owned-relay-tasks

**Given**: A proposal session WebSocket handler with active notification, send, and receive tasks
**When**: Any one of those tasks exits because the socket closes or a channel ends
**Then**: The handler aborts the remaining WebSocket-owned tasks before returning
**And**: the ACP subprocess remains managed by the session rather than by detached WebSocket relay tasks

### Requirement: proposal-session-change-detection

The system shall detect OpenSpec changes generated in the proposal worktree.

#### Scenario: detect-generated-changes

**Given**: A proposal session where the agent has created `openspec/changes/add-auth/proposal.md` in the worktree
**When**: `GET /api/v1/projects/{id}/proposal-sessions/{session_id}/changes` is called
**Then**: The response includes `{ "id": "add-auth" }` with the title parsed from proposal.md
