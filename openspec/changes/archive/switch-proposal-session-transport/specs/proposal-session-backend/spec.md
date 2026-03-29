## ADDED Requirements

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
