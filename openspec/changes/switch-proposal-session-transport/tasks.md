## Implementation Tasks

- [ ] Add an abstraction layer in `src/server/proposal_session.rs` so `ProposalSession` stores an `OpencodeServer` instance and OpenCode `session_id` instead of `Arc<AcpClient>` + ACP session id (verification: `cargo build` compiles with the new struct fields)
- [ ] Update `ProposalSessionManager::create_session` in `src/server/proposal_session.rs` to call `OpencodeServer::spawn`, then `create_session`, and store the resulting server/session handles (verification: session creation unit test passes)
- [ ] Update session shutdown paths in `src/server/proposal_session.rs` (`close_session`, inactivity timeout cleanup, merge cleanup) to call `OpencodeServer::kill` (verification: close-session tests prove the child process is terminated)
- [ ] Replace ACP-specific notification relay logic in `src/server/api.rs:3250-3477` with an OpenCode event adapter that subscribes to SSE and maps `message.part.updated` and `session.status` into the existing WebSocket messages (`agent_message_chunk`, `turn_complete`, etc.) (verification: WebSocket e2e test receives a streamed assistant response and completion event)
- [ ] Replace `send_prompt`, `cancel`, and message-history calls in `src/server/api.rs` with `send_prompt_async`, `abort_session`, and `list_messages` from `OpencodeServer` (verification: chat prompt, cancel, and reconnect-history tests pass)
- [ ] Implement changes-detection path using the existing worktree git logic only; ensure it does not depend on ACP artifacts (verification: `GET /changes` e2e test still passes)
- [ ] Add mock OpenCode Server fixtures under `tests/fixtures/` that expose HTTP endpoints and SSE events equivalent to the needed subset of `opencode serve` (verification: e2e test suite runs without live network or credentials)
- [ ] Update `tests/e2e_proposal_session.rs` to use the mock OpenCode Server transport instead of `mock_acp_agent.py` (verification: `cargo test --test e2e_proposal_session` passes)
- [ ] Keep the dashboard WebSocket protocol unchanged during this change; do not modify frontend message names or payloads yet (verification: dashboard code builds without transport-specific changes)
- [ ] Run `cargo fmt && cargo clippy -- -D warnings && cargo test` to confirm the transport switch is regression-free (verification: all checks pass)

## Future Work

- Remove `src/server/acp_client.rs` after the frontend/state refactor is complete
