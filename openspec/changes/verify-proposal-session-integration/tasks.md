## Implementation Tasks

- [x] Task 1: Create mock ACP binary for testing — minimal executable that speaks ACP JSON-RPC over stdio, responds to `initialize`, `session/create`, `session/prompt` with canned responses, and sends `session/elicitation` on a trigger prompt (file: `tests/fixtures/mock_acp_agent.py`; verification: `python3 tests/fixtures/mock_acp_agent.py` responds to JSON-RPC `initialize` + `session/create`)
- [x] Task 2: Add E2E test — full session lifecycle: create session → WebSocket connect → send prompt → receive agent_message_chunk + turn_complete → list changes → close session (file: `tests/e2e_proposal_session.rs`; verification: `cargo test --test e2e_proposal_session` passes)
- [x] Task 3: Add E2E test — elicitation flow: send prompt that triggers elicitation → verify elicitation message on WebSocket → send accept response → verify agent continues (file: `tests/e2e_proposal_session.rs`; verification: `cargo test --test e2e_proposal_session` passes)
- [x] Task 4: Add E2E test — dirty worktree close: create session → write file in worktree → attempt close without force → verify 409 with file list → close with force → verify cleanup (verification: `cargo test --test e2e_proposal_session` passes)
- [x] Task 5: Add E2E test — merge flow: create session → agent creates proposal.md → commit in worktree → call merge → verify branch merged into base → verify worktree removed (file: `tests/e2e_proposal_session.rs`; verification: `cargo test --test e2e_proposal_session` passes)
- [x] Task 6: Add E2E test — multi-session: create 2 sessions for same project → send different prompts → verify independent responses on each WebSocket → close both (file: `tests/e2e_proposal_session.rs`; verification: `cargo test --test e2e_proposal_session` passes)
- [x] Task 7: Verify WebSocket message type contract — compare TypeScript types in `dashboard/src/api/types.ts` with Rust serde types in `src/server/proposal_session.rs`, fix mismatched session fields and WebSocket payload names so the frontend now matches Rust (`agent_message_chunk` / `tool_call` / `elicitation` / `turn_complete`, plus `timed_out` status and dirty-session fields); verification: `cargo test test_proposal_session_info_serialization` + `cd dashboard && npm run build && npm test -- --run useProposalWebSocket.test.ts` pass
- [x] Task 8: Add UI test — inactivity timeout handling: simulate WebSocket disconnect after timeout → verify UI shows reconnect prompt or error state (file: `dashboard/src/components/__tests__/ProposalChat.test.tsx`; verification: `cd dashboard && npm test -- --run src/components/__tests__/ProposalChat.test.tsx` passes)
- [x] Task 9: Fix any integration issues discovered during E2E testing (verification: aligned frontend/backend proposal session contracts, improved API error parsing, and `cargo test --test e2e_proposal_session` + `cd dashboard && npm test -- --run useProposalWebSocket.test.ts` are green)
- [x] Task 10: Run full CI suite: `cargo fmt --check && cargo clippy -- -D warnings && cargo test && cd dashboard && npm run build && npm run test` (verification: all pass)

## Future Work

- CI pipeline integration for proposal session E2E tests
- Performance benchmarking with concurrent sessions
