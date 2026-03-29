## Implementation Tasks

- [x] Task 1: Add `ProposalSessionConfig` to config types (file: `src/config/types.rs`; fields: `acp_command`, `acp_args`, `acp_env`, `session_inactivity_timeout_secs`; verification: `cargo test` config parsing tests pass with new fields)
- [x] Task 2: Add config defaults and serialization support (file: `src/config/defaults.rs`; verification: default config template includes `proposal_session` section)
- [x] Task 3: Create `src/server/acp_client.rs` — ACP JSON-RPC over stdio client (spawn subprocess, send/receive JSON-RPC messages, handle `initialize` handshake with `elicitation.form` capability; verification: unit test with mock subprocess)
- [x] Task 4: Create `src/server/proposal_session.rs` — session manager struct (`ProposalSessionManager`) with create/list/delete/get operations, storing `ProposalSession` state in `RwLock<HashMap<String, ProposalSession>>` (verification: unit tests for CRUD operations)
- [x] Task 5: Implement worktree creation for proposal sessions — create `proposal/<session_id>` branch from project HEAD, reuse existing `worktree_add` logic (verification: integration test creates worktree at expected path)
- [x] Task 6: Implement ACP session lifecycle — on session create: spawn ACP process in worktree dir, send `initialize`, send `session/create`; on session delete: send `session/close` if possible, kill process (verification: integration test with mock ACP binary)
- [x] Task 7: Implement dirty worktree check on session close — run `git status --porcelain` in worktree, return warning response with file list if dirty, proceed if `force: true` (verification: unit test with dirty/clean worktree scenarios)
- [x] Task 8: Implement inactivity timeout — track `last_activity` timestamp, background task kills ACP process after configured timeout (verification: unit test with accelerated timeout)
- [x] Task 9: Add REST API endpoints to `src/server/api.rs` — `POST/GET /projects/{id}/proposal-sessions`, `DELETE /projects/{id}/proposal-sessions/{session_id}`, `POST .../merge`, `GET .../changes` (verification: `cargo test` API handler tests)
- [x] Task 10: Implement change detection — scan worktree `openspec/changes/*/proposal.md` on demand for `GET .../changes` endpoint (verification: test with proposal.md files in worktree)
- [x] Task 11: Implement merge endpoint — check clean state, `git merge` worktree branch into base, cleanup worktree + ACP process (verification: integration test with mergeable worktree)
- [x] Task 12: Add proposal session WebSocket handler — new route `/proposal-sessions/{session_id}/ws`, proxy messages between client WebSocket and ACP stdio (verification: integration test with mock ACP, message round-trip)
- [x] Task 13: Implement WebSocket message protocol — define client→server types (prompt, elicitation_response, cancel) and server→client types (agent_message_chunk, tool_call, tool_call_update, elicitation, turn_complete, changes_detected, error) (verification: serde round-trip tests for all message types)
- [x] Task 14: Implement ACP `session/prompt` relay — client sends prompt via WebSocket, backend sends `session/prompt` to ACP, streams `session/update` notifications back as WebSocket messages (verification: integration test prompt→response flow)
- [x] Task 15: Implement ACP `session/elicitation` relay — forward elicitation request from ACP to WebSocket client, relay client response back to ACP (verification: integration test elicitation round-trip)
- [x] Task 16: Implement ACP `session/cancel` relay — client sends cancel via WebSocket, backend sends `session/cancel` to ACP (verification: test cancel flow)
- [x] Task 17: Wire `ProposalSessionManager` into `AppState` and router (file: `src/server/api.rs`; verification: `cargo build` succeeds, routes registered)
- [x] Task 18: Add `ProposalSessionManager` cleanup on server shutdown — kill all ACP processes, remove worktrees if clean (verification: test graceful shutdown)
- [x] Task 19: Run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` (verification: all pass)

## Future Work

- ACP session restore after inactivity timeout
- ACP URL-mode elicitation support
- Persistent session storage (survive server restart)
