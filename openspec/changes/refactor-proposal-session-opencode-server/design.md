## Context

The proposal session system currently spawns `opencode acp` (ACP over stdio) per session. This requires a Rust-side JSON-RPC bridge that re-serializes messages to WebSocket for the dashboard. Each layer introduces state-mapping bugs and ACP-spec tracking overhead.

OpenCode 1.3+ ships `opencode serve`, a headless HTTP server with:
- REST endpoints for session/message CRUD (`POST /session`, `POST /session/:id/prompt_async`, `GET /session/:id/message`)
- SSE event stream (`GET /event`) emitting `message.updated`, `message.part.updated`, `session.status`
- Built-in SQLite persistence (messages survive process restarts)
- Official TypeScript SDK (`@opencode-ai/sdk/v2`)

## Goals / Non-Goals

**Goals**:
- Replace ACP stdio bridge with OpenCode Server HTTP/SSE integration
- Eliminate all synthetic ACP compatibility shims
- Fix chat state bugs (message identity, input disable, reconnect restore)
- Simplify the Rust layer to process management + HTTP proxy

**Non-Goals**:
- Remove the Rust server entirely (it still manages worktrees, project registration, orchestration)
- Support multiple AI backends via ACP (if other ACP agents are needed later, add them as a separate capability)
- Dashboard-direct OpenCode Server access without Rust proxy (possible future optimization)

## Decisions

### Decision 1: One `opencode serve` process per proposal session

Each proposal session spawns its own `opencode serve --port 0 --hostname 127.0.0.1` in the worktree directory. The randomly assigned port is captured from stdout and stored with the session. This preserves session isolation and matches the current one-ACP-process-per-session model.

**Alternatives considered**:
- Single shared `opencode serve` for all sessions: simpler process management but OpenCode's project/session model may not map cleanly to multiple worktrees.
- Spawn `opencode serve` at server startup: wastes resources when no proposal sessions are active.

### Decision 2: Rust backend proxies OpenCode Server events to WebSocket

The Rust server subscribes to the OpenCode Server SSE stream and re-emits relevant events to the dashboard WebSocket. This keeps the dashboard protocol unchanged (WebSocket JSON messages) while the backend talks HTTP/SSE to OpenCode.

**Alternatives considered**:
- Dashboard connects directly to OpenCode Server: requires CORS configuration, port discovery, and loses the single-origin security model.

### Decision 3: Chat messages identified by OpenCode message ID

OpenCode Server assigns unique message IDs. The dashboard uses these IDs directly instead of generating `agentMessageId` from timestamps. This fixes the message-overwrite bug at the source.

### Decision 4: Message persistence via OpenCode Server

On session tab reopen, the dashboard requests `GET /session/:id/message` through the Rust proxy to restore conversation history. No separate persistence layer needed in the Rust server.

## Risks / Trade-offs

- **OpenCode Server API stability**: The API is not formally versioned; breaking changes in future OpenCode releases could require updates. Mitigation: pin OpenCode version in deployment and add integration tests.
- **Process overhead**: One Node.js process per session (~50-80MB). Mitigation: sessions already have inactivity timeouts; idle servers are killed.
- **Startup latency**: `opencode serve` takes 1-3s to start vs ~0.5s for `opencode acp`. Mitigation: acceptable for interactive proposal sessions.

## Open Questions

- Should the `@opencode-ai/sdk` TypeScript SDK be used directly in the dashboard (bypassing Rust proxy) for a simpler architecture? Deferred to future work after validating the proxy approach.
