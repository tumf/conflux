# Change: Replace ACP stdio bridge with OpenCode Server HTTP/SSE integration

> **SUPERSEDED** — Split into 4 smaller proposals for implementability:
> 1. `add-opencode-server-client`
> 2. `switch-proposal-session-transport`
> 3. `fix-proposal-chat-state`
> 4. `remove-acp-code`

**Change Type**: implementation

## Why

The current proposal session architecture spawns `opencode acp` as a stdio subprocess and bridges JSON-RPC messages to the dashboard via WebSocket. This has produced a cascade of compatibility issues:

1. **ACP protocol drift** — OpenCode's ACP implementation diverges from the ACP spec (e.g. `session/create` vs `session/new`, `type` vs `sessionUpdate`, missing `turn_complete` notification).
2. **Environment fragility** — `#!/usr/bin/env node` shebang fails under launchd; requires login-shell PATH injection workaround.
3. **Chat state bugs** — Messages overwrite each other (single `agentMessageId` reused across turns), input field stays disabled (synthetic `turn_complete` needed), and conversations are lost on reconnect (in-memory only, no persistence).
4. **Maintenance burden** — Every OpenCode update risks breaking the hand-rolled ACP bridge, requiring ongoing spec-chasing fixes.

OpenCode ships a first-party HTTP server (`opencode serve`) with a complete REST API, SSE event stream, session/message persistence, and an official TypeScript SDK (`@opencode-ai/sdk`). Using this eliminates the ACP bridge entirely.

## What Changes

- **Backend (Rust)**: Replace `acp_client.rs` with `opencode_server.rs` that manages an `opencode serve` subprocess and communicates via HTTP + SSE instead of JSON-RPC over stdio.
- **Frontend (React)**: Replace `useProposalWebSocket.ts` hook with an SSE-based hook consuming OpenCode Server events directly (or via the Rust proxy). Fix chat state management so each assistant turn gets a unique message ID and messages persist across reconnects.
- **Configuration**: Update `proposal_session` config to reflect OpenCode Server options (`port`, `model`, `agent`) instead of ACP options (`acp_command`, `acp_args`).
- **Remove**: `acp_client.rs`, ACP-specific types, synthetic `turn_complete` workaround, login-shell PATH resolution hack.

## Impact

- Affected specs: `proposal-session-backend`, `proposal-session-ui`
- Affected code: `src/server/acp_client.rs`, `src/server/api.rs`, `src/config/types.rs`, `dashboard/src/hooks/useProposalWebSocket.ts`, `dashboard/src/components/ProposalChat.tsx`, `dashboard/src/store/useAppStore.ts`
- **BREAKING**: `proposal_session.acp_command` / `acp_args` / `acp_env` config keys are replaced

## Acceptance Criteria

1. `POST /api/v1/projects/{id}/proposal-sessions` starts an `opencode serve` process and creates a session via its HTTP API.
2. Chat messages sent from the dashboard reach the OpenCode Server session and stream responses back via SSE.
3. Each assistant turn produces a distinct message in the UI (no overwrite).
4. The input field re-enables after the agent finishes responding.
5. Closing and reopening a session tab restores previous messages from the OpenCode Server.
6. No ACP stdio code remains; `acp_client.rs` is deleted.
7. All existing e2e proposal session tests pass (updated to use mock HTTP server).

## Out of Scope

- Replacing the WebSocket protocol between Rust server and dashboard (kept as-is for real-time streaming).
- Multi-project OpenCode Server sharing (one server per session for isolation).
- Authentication (`OPENCODE_SERVER_PASSWORD`) configuration (can be added later).
