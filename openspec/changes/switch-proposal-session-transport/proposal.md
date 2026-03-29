# Change: Switch proposal sessions from ACP to OpenCode transport

**Change Type**: implementation

## Why

Once `OpencodeServer` exists as an isolated module, the backend transport for proposal sessions can switch from ACP stdio to OpenCode Server HTTP/SSE without changing frontend code yet. This keeps the migration focused on the Rust server.

## What Changes

- Update `ProposalSession` and `ProposalSessionManager` to store `OpencodeServer` + OpenCode `session_id` instead of `AcpClient`
- Replace proposal session create/list/close/changes/chat internals in `src/server/proposal_session.rs` and `src/server/api.rs`
- Keep the dashboard WebSocket message contract unchanged (`agent_message_chunk`, `tool_call`, `turn_complete`, etc.)
- Add backend tests that exercise the new transport via mock HTTP/SSE instead of mock ACP stdio

## Impact

- Affected specs: `proposal-session-backend`
- Affected code: `src/server/proposal_session.rs`, `src/server/api.rs`, `src/server/acp_client.rs`, `tests/e2e_proposal_session.rs`, `tests/fixtures/mock_opencode_server.*`
- Behavior changes only in backend transport; frontend protocol remains stable

## Acceptance Criteria

1. `POST /api/v1/projects/{id}/proposal-sessions` uses `OpencodeServer::spawn` and `create_session`
2. WebSocket chat works end-to-end using OpenCode Server events
3. `GET /api/v1/projects/{id}/proposal-sessions/{session_id}/changes` still works
4. `DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}` kills the OpenCode Server process
5. `tests/e2e_proposal_session.rs` passes without ACP mocks
