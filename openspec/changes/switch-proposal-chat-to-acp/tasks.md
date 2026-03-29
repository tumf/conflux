## Implementation Tasks

- [ ] Restore ACP-backed proposal session state in `src/server/proposal_session.rs` by replacing `OpencodeServer`/OpenCode session fields with `AcpClient`/ACP session identifiers and spawning ACP with `--cwd <worktree_path>` (verification: `cargo build` succeeds and `tests/e2e_proposal_session.rs` creates a session against the ACP mock).
- [ ] Replace OpenCode SSE relay logic in `src/server/api.rs` with ACP notification relay logic that preserves the existing dashboard WebSocket message contract for assistant chunks, tool calls, tool-call updates, elicitation, and turn completion (verification: `cargo test --test e2e_proposal_session`).
- [ ] Restore ACP-backed prompt, cancel, elicitation-response, and history hydration flows in `src/server/api.rs` using `AcpClient` request/notification methods instead of OpenCode HTTP calls (verification: `cargo test --test e2e_proposal_session proposal_session_ws_cancel_and_reconnect_history_work -- --nocapture`).
- [ ] Reset proposal-session transport defaults and documentation in `src/config/defaults.rs`, `src/config/types.rs`, and related config-loading tests to ACP startup semantics (`opencode`, `acp`) while preserving backward-compatible aliases where needed (verification: `cargo test config::`).
- [ ] Replace OpenCode Server-specific test fixtures with ACP stdio fixtures in `tests/fixtures/` and update `tests/e2e_proposal_session.rs` to validate ACP-driven proposal chat behavior end-to-end (verification: `cargo test --test e2e_proposal_session`).
- [ ] Remove `src/server/opencode_client.rs` and any remaining proposal-session references to OpenCode transport code after ACP coverage is restored (verification: `cargo build` and `rg -n "OpencodeServer|opencode_client" src tests openspec/specs/proposal-session-backend/spec.md` only returns intentional historical/archive references).
- [ ] Run repository verification for the backend transport switch (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`) and fix any regressions before completion (verification: all three commands exit successfully).

## Future Work

- Manually exercise proposal chat from the server-mode WebUI against a real `opencode acp` binary after merge to confirm parity with the mocked ACP e2e flow.
