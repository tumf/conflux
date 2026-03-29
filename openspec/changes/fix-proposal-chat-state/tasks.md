## Implementation Tasks

- [x] Replace the single `agentMessageId = `agent-${session.id}`` model in `dashboard/src/components/ProposalChat.tsx:48-89` with explicit per-turn message identity created when a prompt is submitted or when the first assistant chunk arrives (verification: no code path reuses a fixed assistant ID per session)
- [x] Refactor `dashboard/src/store/useAppStore.ts` to store committed chat messages separately from active streaming turn state, for example by introducing `activeTurnBySessionId` or equivalent state instead of writing directly into `streamingContent[agentMessageId]` (verification: reducer tests prove a second turn does not overwrite the first assistant message)
- [x] Update `APPEND_STREAMING_CHUNK`, `APPEND_CHAT_MESSAGE`, and related actions in `dashboard/src/store/useAppStore.ts:234-267` so that turn completion commits the active streamed content into a new assistant message and clears only the active turn state (verification: reducer test for two sequential turns passes)
- [x] Update `dashboard/src/hooks/useProposalWebSocket.ts` callbacks so `onTurnComplete` and `onError` drive explicit state transitions rather than relying on implicit `message.role === 'user'` toggling for `isAgentResponding` (verification: component test proves input re-enables after completion and after error)
- [x] Add a history-loading path when `ProposalChat` mounts or when `session.id` becomes active, using the backend history endpoint to populate `chatMessagesBySessionId[session.id]` before new streaming begins (verification: integration/component test proves reopening the same session restores prior messages)
- [x] Update `dashboard/src/api/types.ts` if needed so message/turn metadata can carry stable IDs and hydration markers (verification: TypeScript build passes without casts)
- [x] Add dashboard tests (store reducer tests and/or React component tests) covering: sequential turns, tool calls during a turn, reconnect history restore, and input disable/re-enable semantics (verification: frontend test command passes)
- [x] Run the dashboard test/build pipeline and repo checks (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`) to confirm the UI state refactor does not regress server code (verification: all checks pass)

## Future Work

- Replace the custom hook with direct `@opencode-ai/sdk` integration if the backend proxy is later removed
