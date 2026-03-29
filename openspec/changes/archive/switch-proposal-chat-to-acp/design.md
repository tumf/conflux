## Context

The proposal-session backend currently contains two competing transport models:
- canonical OpenSpec requirements and the retained `AcpClient` favor ACP stdio JSON-RPC
- current runtime code favors `opencode serve` over HTTP/SSE via `OpencodeServer`

The requested direction is to make ACP canonical again and remove the OpenCode Server transport entirely from proposal chat.

## Goals / Non-Goals

### Goals
- Make ACP the single runtime transport for proposal-session chat.
- Keep the dashboard WebSocket contract stable.
- Pass the proposal worktree explicitly to ACP via `--cwd`.
- Remove the now-obsolete OpenCode Server client and associated transport-specific tests/spec drift.

### Non-Goals
- Supporting both ACP and OpenCode Server as selectable transports.
- Redesigning dashboard chat state or UI behavior.
- Changing proposal-session worktree creation/merge semantics unrelated to chat transport.

## Decisions

### Decision: Reuse `AcpClient` as the single transport adapter
`src/server/acp_client.rs` already implements ACP subprocess spawning, JSON-RPC request/response handling, and typed `session/update` event parsing. Restoring it avoids a second transport abstraction and aligns runtime behavior with the project specs.

### Decision: Pass the worktree via explicit `--cwd`
The ACP command line will include `--cwd <worktree_path>` in addition to process spawning in the worktree context. This makes the intended repository root explicit to the agent process and matches the requested operational contract.

### Decision: Preserve the dashboard WebSocket message shapes
The dashboard already expects messages such as `agent_message_chunk`, `tool_call`, `tool_call_update`, `elicitation`, and `turn_complete`. The backend will continue translating transport-native events into those shapes so no frontend protocol migration is required.

### Decision: Remove `OpencodeServer` instead of keeping a dormant fallback
Keeping an unused second transport would preserve ambiguity in config, tests, and specs. Since the requested direction is ACP-only, proposal chat should delete the OpenCode transport code rather than leave an unofficial fallback path.

## Risks / Trade-offs

- ACP history hydration may not map one-to-one with the current OpenCode message-history APIs, so reconnect behavior must be revalidated carefully.
- Deleting `opencode_client.rs` reduces future transport flexibility, but it also removes current architectural drift and maintenance overhead.
- Re-enabling elicitation over ACP may expose latent assumptions in the WebSocket flow that were masked while OpenCode transport returned a not-supported error.

## Migration Plan

1. Replace proposal-session runtime references to `OpencodeServer` with `AcpClient`.
2. Update server WebSocket relay paths to use ACP notifications and request methods.
3. Update config defaults/tests and backend specs to ACP terminology.
4. Replace e2e fixtures with ACP mocks and verify reconnect/cancel/elicitation flows.
5. Remove `opencode_client.rs` once no live references remain.

## Open Questions

- None for proposal scope; the requested runtime contract and cleanup target were explicitly specified in the session.
