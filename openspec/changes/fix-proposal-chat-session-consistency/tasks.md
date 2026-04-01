## Implementation Tasks

- [x] 1. Define a single authoritative history-restoration model for proposal chat initial load vs reconnect replay, and update the relevant UI/backend spec deltas to remove ambiguity (verification: `openspec/changes/fix-proposal-chat-session-consistency/specs/proposal-session-ui/spec.md` and `openspec/changes/fix-proposal-chat-session-consistency/specs/proposal-session-backend/spec.md` describe one non-duplicating restoration contract).
- [x] 2. Update `dashboard/src/hooks/useProposalChat.ts` so session-scoped async history loads and WebSocket events are discarded when they belong to a stale `projectId/sessionId` generation (verification: hook tests cover session switch during in-flight history load and prove the newer session state is not overwritten).
- [x] 3. Update proposal chat message reconciliation so reload/reconnect cannot append duplicate logical assistant turns when history hydration and replay both occur (verification: frontend tests cover reload/reconnect and assert no duplicate assistant messages for one logical turn).
- [x] 4. Update `src/server/api.rs` and/or `src/server/proposal_session.rs` replay contract so replayed assistant/tool-call events carry enough stable identity for client-side deduplication/reconciliation, or otherwise enforce a single-source hydration boundary (verification: `tests/e2e_proposal_session.rs` or equivalent server tests assert deterministic replay semantics on reconnect).
- [x] 5. Add regression coverage for cross-session isolation and idempotent history restoration across REST load, reconnect replay, and browser reload (verification: proposal chat hook/component tests plus backend/integration tests cover duplicate-prevention and session-isolation scenarios).

## Future Work

- Verify whether the same session-generation / stale-response isolation pattern should also be applied to terminal WebSocket views.
- Consider consolidating all dashboard real-time session views onto a shared event reconciliation abstraction if proposal chat and terminal chat continue to diverge.
