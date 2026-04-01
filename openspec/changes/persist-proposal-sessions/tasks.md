## Implementation Tasks

### Phase 1: SQLite Schema and ServerDb Methods

- [ ] 1.1 Add migration v2 to `src/server/db.rs`: create `ui_state`, `proposal_sessions`, `proposal_session_messages` tables; bump `SCHEMA_VERSION` to 2 (verification: `cargo test test_server_db` passes)
- [ ] 1.2 Add `ServerDb` methods for ui_state: `get_ui_state`, `set_ui_state`, `delete_ui_state`, `get_all_ui_state` (verification: new unit test `test_ui_state_crud` in `db.rs`)
- [ ] 1.3 Add `ServerDb` methods for proposal sessions: `upsert_proposal_session`, `update_proposal_session_status`, `update_proposal_session_activity`, `load_active_proposal_sessions`, `delete_proposal_session` (verification: new unit test `test_proposal_session_crud` in `db.rs`)
- [ ] 1.4 Add `ServerDb` methods for session messages: `insert_proposal_session_message`, `load_proposal_session_messages`, `delete_proposal_session_messages` (verification: new unit test `test_proposal_session_messages_crud` in `db.rs`)

### Phase 2: ProposalSessionManager Persistence

- [ ] 2.1 Add `db: Option<Arc<ServerDb>>` field to `ProposalSessionManager`; update `new()` and `create_proposal_session_manager()` signatures (verification: `cargo build` succeeds)
- [ ] 2.2 Persist session on creation: call `db.upsert_proposal_session()` after successful `create_session()` (verification: integration test or manual DB inspection)
- [ ] 2.3 Persist messages on write: call `db.insert_proposal_session_message()` in `record_user_prompt`, `append_assistant_chunk_with_kind` (on turn complete), and `complete_active_turn` (verification: messages appear in DB after chat interaction)
- [ ] 2.4 Delete session from DB on `close_session()` and `merge_session()`: call `db.delete_proposal_session()` and `db.delete_proposal_session_messages()` (verification: DB rows removed after close/merge)
- [ ] 2.5 Update session status in DB on `scan_timeouts()`: call `db.update_proposal_session_status(id, "timed_out")` (verification: status updated in DB after timeout)
- [ ] 2.6 Throttle activity writes: add `last_db_activity_write` field to `ProposalSession`, skip DB write if < 60s since last write (verification: code review)

### Phase 3: Server Startup Session Restoration

- [ ] 3.1 In `run_server()` (`src/server/mod.rs`), after creating `ProposalSessionManager`, call `db.load_active_proposal_sessions()` and restore each session (verification: sessions restored after server restart with existing worktrees)
- [ ] 3.2 For each restored session: validate worktree path exists, re-spawn ACP subprocess, load message history from DB (verification: manual test — restart server, verify session tab reappears)
- [ ] 3.3 Remove stale sessions from DB when worktree no longer exists (verification: DB row deleted for missing worktree)

### Phase 4: UI State REST API

- [ ] 4.1 Add `GET /api/v1/ui-state` endpoint returning all ui_state as JSON object (verification: `curl` returns stored state)
- [ ] 4.2 Add `PUT /api/v1/ui-state/{key}` endpoint to set a value (verification: `curl -X PUT` stores value)
- [ ] 4.3 Add `DELETE /api/v1/ui-state/{key}` endpoint to remove a value (verification: `curl -X DELETE` removes value)
- [ ] 4.4 Extend `FullState` WebSocket payload with `ui_state: HashMap<String, String>` field (verification: WebSocket message includes `ui_state`)

### Phase 5: Dashboard Frontend Restoration

- [ ] 5.1 Add `ui_state` field to `FullState` type in `dashboard/src/api/types.ts` (verification: TypeScript compiles)
- [ ] 5.2 Add `setUiState(key, value)` and `deleteUiState(key)` functions to `dashboard/src/api/restClient.ts` (verification: TypeScript compiles)
- [ ] 5.3 In `App.tsx`, restore `selectedProjectId` from `ui_state` on `SET_FULL_STATE` when the project exists in the project list (verification: browser reload preserves project selection)
- [ ] 5.4 In `App.tsx`, restore `activeProposalSessionId` from `ui_state` after session list fetch when the session is still active (verification: browser reload preserves session tab)
- [ ] 5.5 Fire-and-forget `setUiState` on project selection and session selection; `deleteUiState` on null selection (verification: UI state persisted on interaction)
- [ ] 5.6 Validate and clean stale references: if restored session/project ID not found, call `deleteUiState` (verification: stale entries cleaned)

### Phase 6: Lint, Format, Test

- [ ] 6.1 Run `cargo fmt --check` and `cargo clippy -- -D warnings` — fix any issues (verification: both pass cleanly)
- [ ] 6.2 Run `cargo test` — all existing and new tests pass (verification: zero failures)
- [ ] 6.3 Run `cd dashboard && npm run build` — frontend builds without errors (verification: build succeeds)

## Future Work

- Multi-user session isolation if server becomes multi-tenant
- ACP conversation state persistence (would require ACP protocol support for session resume)
- Session expiry policy (auto-delete sessions older than N days)
