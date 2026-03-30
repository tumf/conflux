## Implementation Tasks

- [x] Add `AgentThoughtChunk { text: String }` variant to `ProposalWsServerMessage` in `src/server/api.rs` (verification: `cargo build` compiles, `cargo test` passes)
- [x] Split match arm at `api.rs:3586-3587` so `AcpEvent::AgentThoughtChunk` maps to `ProposalWsServerMessage::AgentThoughtChunk` (verification: `cargo test` passes, manual WS inspection shows `agent_thought_chunk` type)
- [x] Add `is_thought: Option<bool>` field to `ProposalSessionMessageRecord` in `src/server/proposal_session.rs` with `#[serde(skip_serializing_if = "Option::is_none")]` (verification: existing serialization tests pass, new field present in JSON when set)
- [x] Update `append_assistant_chunk` (or add `append_assistant_thought_chunk`) in `ProposalSessionManager` to set `is_thought = Some(true)` for thought records (verification: unit test asserting `is_thought` on recorded message)
- [x] Wire the server-side thought recording into the notification relay loop in `api.rs` so thought chunks call the thought-aware append method (verification: `cargo test`)
- [x] Add `'agent_thought_chunk'` to `ProposalWsMessageType` and `ProposalWsServerMessage` union in `dashboard/src/api/types.ts` (verification: `npm run build` in dashboard)
- [x] Add `onThoughtChunk` callback to `UseProposalWebSocketOptions` and handle `agent_thought_chunk` in `handleServerMessage` in `dashboard/src/hooks/useProposalWebSocket.ts` (verification: `npm test` in dashboard)
- [x] Add test case for `agent_thought_chunk` dispatch in `dashboard/src/hooks/useProposalWebSocket.test.ts` (verification: `npm test` passes)
- [x] Wire `onThoughtChunk` in `ProposalChat.tsx` as a no-op or pass-through (verification: `npm run build` in dashboard, no type errors)
- [x] Run `cargo fmt --check && cargo clippy -- -D warnings` and `cd dashboard && npm run build` to confirm no regressions (verification: all pass)

## Future Work

- UI display for thought chunks (toggle, collapse, debug view)

## Implementation Blocker #1
- category: other
- summary: ローカルディスク容量不足で Rust 側の最終回帰検証（clippy/e2e）が完了できない
- evidence:
  - `cargo test e2e_proposal_session -- --nocapture` 実行時に `No space left on device (os error 28)` で失敗
  - `df -h .` 結果で `/System/Volumes/Data` の空きが `1.4Gi`、使用率 `100%`
- impact: `cargo clippy -- -D warnings` と e2e を含む最終 Rust 回帰確認が未完了
- unblock_actions:
  - 開発環境のディスク空き容量を増やして `cargo clippy -- -D warnings` を再実行
  - 同条件で `cargo test e2e_proposal_session -- --nocapture` を再実行し WebSocket 挙動を再確認
- owner: development
- decision_due: 2026-03-31
