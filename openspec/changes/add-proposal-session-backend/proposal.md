# Change: Proposal Session Backend — ACP Client, API Endpoints, Session Lifecycle

## Problem / Context

Conflux Dashboard currently has no proposal creation workflow. Users must use the TUI `propose_command` or manually create `openspec/changes/` directories. The Dashboard needs an interactive chat-based proposal creation flow backed by ACP (Agent Client Protocol), where an AI agent generates OpenSpec changes through conversation with the user.

## Proposed Solution

Add a proposal session subsystem to the Conflux server that:

1. **Config**: Add `proposal_session` config section to `.cflx.jsonc` with ACP command, args, env, and timeout settings.
2. **ACP Client**: Implement a JSON-RPC over stdio ACP client that manages `opencode acp` subprocesses, handles `initialize`, `session/create`, `session/prompt`, `session/update`, `session/elicitation`, and `session/cancel`.
3. **API Endpoints**: REST + WebSocket endpoints for creating/listing/deleting sessions, merging worktrees, listing generated changes, and real-time chat communication.
4. **Session Lifecycle**: Each session creates an independent worktree (branch: `proposal/<timestamp-or-id>`), spawns an ACP subprocess, and proxies JSON-RPC messages to/from the Dashboard WebSocket. Multiple sessions per project are allowed.
5. **Dirty Worktree Handling**: Session close checks `git status`; if dirty, returns a warning. User can force-close with `force: true`.
6. **Commit Policy**: Conflux does not perform commits. Users commit via terminal or agent instruction.

## Acceptance Criteria

- `proposal_session` config fields are parsed and validated from `.cflx.jsonc`
- `POST /api/v1/projects/{id}/proposal-sessions` creates worktree + ACP subprocess and returns session info
- `GET /api/v1/projects/{id}/proposal-sessions` lists active sessions for a project
- `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` checks dirty state, returns warning or performs cleanup
- `DELETE` with `force: true` closes even when dirty
- `POST /api/v1/projects/{id}/proposal-sessions/{session_id}/merge` merges clean worktree to base and cleans up
- `GET /api/v1/projects/{id}/proposal-sessions/{session_id}/changes` lists detected changes
- WebSocket at `/proposal-sessions/{session_id}/ws` relays ACP prompt/response/elicitation messages
- ACP `session/elicitation` (form mode) messages are forwarded to client and responses relayed back
- Session inactivity timeout stops ACP process (configurable, default 1800s)
- Multiple concurrent sessions per project work independently

## Out of Scope

- Dashboard UI (separate proposal: `add-proposal-session-ui`)
- ACP URL-mode elicitation (form mode only for initial version)
- Auto-commit by Conflux
- ACP session restore/resume after timeout
