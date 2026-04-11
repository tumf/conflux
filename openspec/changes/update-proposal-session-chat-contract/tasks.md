## Implementation Tasks

- [ ] Canonicalize proposal-session chat specs around WebSocket-only hydration and ACK-based submission locking (verification: `openspec/changes/update-proposal-session-chat-contract/specs/proposal-session-backend/spec.md`, `.../proposal-session-ui/spec.md`, and `.../proposal-session-integration/spec.md` no longer require REST hydration and agree on disable/failed/retry semantics)
- [ ] Remove REST-based proposal-session history hydration from the Dashboard hook and rely on WebSocket replay/recovery for initial message restoration (verification: `dashboard/src/hooks/useProposalChat.ts` no longer imports or calls `listProposalSessionMessages`)
- [ ] Implement explicit failed-send and retry state handling for proposal-session user messages while keeping the input locked until server ACK (verification: `dashboard/src/hooks/useProposalChat.ts`, `dashboard/src/components/ProposalChat.tsx`, and `dashboard/src/components/ChatMessageList.tsx` expose `pending` / `failed` / retry behavior with ACK-driven unlock)
- [ ] Add or update dashboard tests covering WebSocket-only hydration, ACK-based unlock, failed send transitions, and retry behavior (verification: `dashboard/src/hooks/useProposalChat.test.ts` and related component tests cover the canonical flows without REST history assumptions)
- [ ] Run proposal validation and repository quality gates for the implementation (verification: `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate update-proposal-session-chat-contract --strict`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm --prefix dashboard run lint`, `npm --prefix dashboard run test`)

## Future Work

- Evaluate whether the REST message history endpoint should remain only for debugging/admin inspection once Dashboard hydration no longer depends on it.
