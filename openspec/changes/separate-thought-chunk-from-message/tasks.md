## Implementation Tasks

- [ ] Add `AgentThoughtChunk { text: String }` variant to `ProposalWsServerMessage` in `src/server/api.rs` (verification: `cargo build` compiles, `cargo test` passes)
- [ ] Split match arm at `api.rs:3586-3587` so `AcpEvent::AgentThoughtChunk` maps to `ProposalWsServerMessage::AgentThoughtChunk` (verification: `cargo test` passes, manual WS inspection shows `agent_thought_chunk` type)
- [ ] Add `is_thought: Option<bool>` field to `ProposalSessionMessageRecord` in `src/server/proposal_session.rs` with `#[serde(skip_serializing_if = "Option::is_none")]` (verification: existing serialization tests pass, new field present in JSON when set)
- [ ] Update `append_assistant_chunk` (or add `append_assistant_thought_chunk`) in `ProposalSessionManager` to set `is_thought = Some(true)` for thought records (verification: unit test asserting `is_thought` on recorded message)
- [ ] Wire the server-side thought recording into the notification relay loop in `api.rs` so thought chunks call the thought-aware append method (verification: `cargo test`)
- [ ] Add `'agent_thought_chunk'` to `ProposalWsMessageType` and `ProposalWsServerMessage` union in `dashboard/src/api/types.ts` (verification: `npm run build` in dashboard)
- [ ] Add `onThoughtChunk` callback to `UseProposalWebSocketOptions` and handle `agent_thought_chunk` in `handleServerMessage` in `dashboard/src/hooks/useProposalWebSocket.ts` (verification: `npm test` in dashboard)
- [ ] Add test case for `agent_thought_chunk` dispatch in `dashboard/src/hooks/useProposalWebSocket.test.ts` (verification: `npm test` passes)
- [ ] Wire `onThoughtChunk` in `ProposalChat.tsx` as a no-op or pass-through (verification: `npm run build` in dashboard, no type errors)
- [ ] Run `cargo fmt --check && cargo clippy -- -D warnings` and `cd dashboard && npm run build` to confirm no regressions (verification: all pass)

## Future Work

- UI display for thought chunks (toggle, collapse, debug view)
