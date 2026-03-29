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

## Implementation Blocker #1
- category: spec_contradiction
- summary: 前提条件である `OpencodeServer` 実装がリポジトリに存在せず、本変更単体では OpenCode transport 置換タスクを開始できない
- evidence:
  - `openspec/changes/switch-proposal-session-transport/proposal.md:7` に「Once `OpencodeServer` exists」と前提が明記されている
  - `openspec/changes/switch-proposal-session-transport/tasks.md:3-7` が `OpencodeServer::spawn/create_session/send_prompt_async/abort_session/list_messages` の既存実装を前提としている
  - `src/server/mod.rs:11-17` に `opencode_client` モジュールが存在せず `acp_client` のみ公開されている
  - `src/server/proposal_session.rs:18,66-67,171-185` が `AcpClient` と ACP session ID にハード依存している
  - `src/server/api.rs:3216-3477` が `crate::server::acp_client::*` 型と `session/update` 通知に直接依存している
- impact: `ProposalSession`/`api.rs` を OpenCode transport へ差し替える実装タスク（Implementation Tasks 1-8）を、仕様どおりの依存順で完了できない
- unblock_actions:
  - `add-opencode-server-client` 相当の `src/server/opencode_client.rs`（spawn/health/create_session/send_prompt_async/list_messages/abort_session/subscribe_events/kill）を先に実装・統合する
  - `ProposalSessionConfig` を ACP 命名から transport 非依存命名へ更新し、サーバ設定とテスト fixture を OpenCode 側に合わせる
- owner: backend-maintainer
- decision_due: 2026-03-31

- owner: backend-maintainer
- decision_due: 2026-03-31
