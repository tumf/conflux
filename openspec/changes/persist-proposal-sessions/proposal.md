---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/db.rs
  - src/server/proposal_session.rs
  - src/server/mod.rs
  - src/server/api.rs
  - dashboard/src/store/useAppStore.ts
  - dashboard/src/App.tsx
  - dashboard/src/api/restClient.ts
  - dashboard/src/api/types.ts
---

# Change: Persist proposal sessions and UI state to SQLite

**Change Type**: implementation

## Why

Browser reload loses the active proposal chat session ("Session is not active" error) because both frontend selection state (`selectedProjectId`, `activeProposalSessionId`) and backend session data (`ProposalSessionManager.sessions` HashMap) are purely in-memory. Server restart also destroys all sessions even when worktrees still exist on disk.

## What Changes

- **SQLite schema v2**: Add `ui_state` key-value table and `proposal_sessions` / `proposal_session_messages` tables to `cflx.db`
- **ServerDb methods**: CRUD for ui_state, proposal sessions, and session messages
- **ProposalSessionManager**: Accept `Option<Arc<ServerDb>>` via DI; persist session lifecycle events and chat messages to SQLite
- **Server startup**: Restore surviving sessions from SQLite (re-spawn ACP subprocesses for worktrees that still exist on disk)
- **REST API**: Add `GET/PUT/DELETE /api/v1/ui-state/{key}` endpoints; extend `FullState` WebSocket payload with `ui_state` map
- **Dashboard frontend**: Save/restore `selectedProjectId` and `activeProposalSessionId` via the ui-state API on project/session selection and on initial FullState load

## Impact

- Affected specs: `server-persistence`, `proposal-session`, `dashboard-api`
- Affected code: `src/server/db.rs`, `src/server/proposal_session.rs`, `src/server/mod.rs`, `src/server/api.rs`, `dashboard/src/api/types.ts`, `dashboard/src/api/restClient.ts`, `dashboard/src/store/useAppStore.ts`, `dashboard/src/App.tsx`

## Acceptance Criteria

1. Browser reload with an active proposal session restores the project selection, session tab, and chat history without errors
2. Server restart with surviving worktrees restores sessions (ACP re-spawned, messages loaded from DB)
3. Server restart with missing worktrees (manually deleted) gracefully removes stale sessions from DB
4. TimedOut sessions are preserved across restarts and become Active when re-accessed
5. Closed/Merged sessions are deleted from DB along with their messages
6. `cargo test` passes with new unit tests for all ServerDb CRUD methods
7. `cargo clippy -- -D warnings` and `cargo fmt --check` pass

## Out of Scope

- Session-level encryption or access control for the SQLite data
- Multi-user session isolation (server is single-user by design)
- ACP conversation state restoration (ACP subprocess gets a fresh session; only chat message display history is preserved)
