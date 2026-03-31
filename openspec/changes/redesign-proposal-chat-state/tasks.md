## Implementation Tasks

- [ ] 1. Add `client_message_id` to Rust WebSocket protocol: Add `client_message_id: Option<String>` with `#[serde(default)]` to `ProposalWsClientMessage::Prompt` and with `#[serde(skip_serializing_if = "Option::is_none")]` to `ProposalWsServerMessage::UserMessage` in `src/server/api.rs`. Pass `client_message_id` through in `proposal_session_ws` recv_task Prompt handler. (verification: `cargo test --test e2e_proposal_session` passes; new field round-trips in test)

- [ ] 2. Create `useProposalChat` hook in `dashboard/src/hooks/useProposalChat.ts`: Implement `status` state machine (`ready → submitted → streaming → ready | error`). Manage `messages: Message[]` as single source of truth with in-place streaming on last assistant message. On `user_message` from server, match by `client_message_id` to replace optimistic entry. On `turn_complete`, finalize and transition to `ready`. On WS error/disconnect during turn, transition to `error`. Expose `{ messages, status, sendMessage, stop, error, activeElicitation, sendElicitationResponse, wsConnected }`. (verification: unit tests in `dashboard/src/hooks/useProposalChat.test.ts`)

- [ ] 3. Add WebSocket reconnection to `useProposalChat`: Exponential backoff with delays [1000, 2000, 4000, 8000, 16000]ms, max 30000ms, max 10 retries. On reconnect success, flush pending prompts and replay history from server. On disconnect during active turn, fail the turn and set status to `error`. Reset reconnect counter on successful connection. (verification: unit test simulating disconnect/reconnect cycle)

- [ ] 4. Add double-send guard to `ChatInput`: Clear `value` synchronously before calling `onSend`. Guard with `if (!trimmed || disabled) return` where `disabled` is `status !== 'ready'`. Remove `disabled` prop from textarea element and apply only to send button. Update placeholder text based on status. (verification: `dashboard/src/components/__tests__/ChatInput.test.tsx` updated)

- [ ] 5. Remove chat state from `useAppStore`: Remove state fields `chatMessagesBySessionId`, `streamingContent`, `activeTurnBySessionId`, `isAgentResponding`, `activeElicitation`. Remove actions `APPEND_CHAT_MESSAGE`, `UPSERT_SERVER_USER_MESSAGE`, `UPDATE_CHAT_MESSAGE_SEND_STATUS`, `START_ASSISTANT_TURN`, `APPEND_STREAMING_CHUNK`, `COMPLETE_ASSISTANT_TURN`, `FAIL_ASSISTANT_TURN`, `UPDATE_TOOL_CALL`, `UPDATE_TOOL_CALL_STATUS`, `SET_AGENT_RESPONDING`, `SET_ELICITATION`, `HYDRATE_CHAT_MESSAGES` and corresponding useCallback wrappers. (verification: `dashboard/src/store/useAppStore.test.ts` updated and passes)

- [ ] 6. Refactor `ProposalChat` to use `useProposalChat` hook: Replace 17+ callback props with hook usage inside component. Props reduced to `projectId`, `sessionId`, `onBack`, `onMerge`, `onClose`, `onClickChange`, `isLoading`. Wire `ChatMessageList`, `ChatInput`, and `ElicitationDialog` to hook state. (verification: `dashboard/src/components/__tests__/ProposalChat.test.tsx` updated)

- [ ] 7. Update `App.tsx` to remove chat state threading: Remove all chat-related store callbacks passed to `ProposalChat`. Pass only `projectId`, `sessionId`, `onBack`, `onMerge`, `onClose`, `onClickChange`, `isLoading`. (verification: `npm run build` succeeds with no type errors)

- [ ] 8. Delete `useProposalWebSocket.ts` and its tests: Remove `dashboard/src/hooks/useProposalWebSocket.ts` and `dashboard/src/hooks/useProposalWebSocket.test.ts`. Ensure no imports reference the deleted files. (verification: `npm run build` succeeds)

- [ ] 9. Update e2e proposal session tests: Update `tests/e2e_proposal_session.rs` to include `client_message_id` in prompt messages where applicable. Verify `user_message` response includes `client_message_id` when sent and omits it when not sent (backward compatibility). (verification: `cargo test --test e2e_proposal_session` passes)

- [ ] 10. Run full lint and test suite: `cd dashboard && npm run build`, `cd dashboard && npm test`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`. (verification: all commands exit 0)

## Future Work

- Server-side streaming resume on reconnect (send accumulated content in `session_state` message)
- Multi-session concurrent streaming support
- Migrate remaining hardcoded hex colors to semantic tokens (separate proposal)
