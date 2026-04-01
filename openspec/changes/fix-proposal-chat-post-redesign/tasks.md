## Implementation Tasks

- [ ] 1. Add REST messages endpoint to server: Create `async fn get_proposal_session_messages()` handler in `src/server/api.rs` that calls `manager.list_messages(&session_id)` and returns JSON array. Register `GET /projects/{id}/proposal-sessions/{session_id}/messages` in the API router alongside existing session routes. (verification: `cargo test` passes; manual curl returns 200 with message array)

- [ ] 2. Fix typing indicator in ChatMessageList: Remove `streamingContent` prop and `streamingIds` derived state from `ChatMessageList`. Change `showTypingIndicator` logic to: `isAgentResponding && (messages.length === 0 || messages[messages.length - 1].role !== 'assistant')`. Remove the orphan streaming block that renders `streamingContent` entries not in `messages`. (verification: `dashboard/src/components/__tests__/ChatMessageList.test.tsx` updated; typing indicator hides when assistant message appears)

- [ ] 3. Update ProposalChat to stop passing streamingContent: Remove `streamingContent={{}}` prop from `ChatMessageList` usage in `ProposalChat.tsx`. (verification: `npm run build` succeeds with no type errors)

- [ ] 4. Transition status to streaming on tool_call: In `useProposalChat.ts` `handleServerMessage` case `'tool_call'`, add `transitionStatus('streaming', 'tool_call')` after updating the tool call. This ensures status leaves `submitted` when the agent uses tools without emitting text chunks. (verification: unit test confirming status transitions to `streaming` on `tool_call` event)

- [ ] 5. Fix REST/WS history race in useProposalChat: In the `useEffect` that runs on session change, await `listProposalSessionMessages` completion before calling `connect()`. Add a ref flag `historyLoadedRef` set to true after REST load. In WS `onopen`, only call `flushPendingPrompts()` (replay comes from REST, not WS). Alternatively, guard `setMessages` from REST to not overwrite if WS has already delivered messages. (verification: unit test simulating reload scenario; messages persist)

- [ ] 6. Run full lint and test suite: `cd dashboard && npm run build`, `cd dashboard && npm test`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`. (verification: all commands exit 0)

## Future Work

- Server-side streaming resume on reconnect
- Remove WS-based replay in `build_replay_ws_messages` once REST endpoint handles all history needs
