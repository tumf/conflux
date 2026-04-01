---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/proposal-session-ui/spec.md
  - openspec/specs/proposal-ws-streaming/spec.md
  - dashboard/src/hooks/useProposalWebSocket.ts
  - dashboard/src/store/useAppStore.ts
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/components/ChatInput.tsx
  - dashboard/src/components/ChatMessageList.tsx
  - src/server/api.rs
---

# Change: Redesign Proposal Chat State Management Following useChat Pattern

**Change Type**: implementation

## Why

The current proposal chat implementation suffers from multiple fundamental design
issues that cannot be fixed individually. The root cause is that the state
management deviates from well-established AI chat UI patterns (Vercel AI SDK
`useChat`, assistant-ui). Specifically:

1. **Message state is fragmented across 3 stores** (`chatMessagesBySessionId`,
   `streamingContent`, `activeTurnBySessionId`) instead of a single `messages[]`
   array as the sole source of truth.
2. **Turn status uses 4 independent boolean/enum flags** (`isAgentResponding`,
   WS `status`, per-message `sendStatus`, `activeTurnBySessionId`) that
   desynchronise, instead of a single `status` state machine
   (`ready → submitted → streaming → ready | error`).
3. **Client message IDs are not round-tripped** through the WebSocket protocol,
   so the server's `user_message` echo cannot be correlated with the optimistic
   message — causing duplicates on Enter double-tap and stale "Queued" badges.
4. **Proposal WebSocket has no reconnection logic**, unlike the main dashboard
   WebSocket — a single disconnect permanently blocks the UI.
5. **`disabled` prop disables the textarea itself** rather than only the send
   button, which is non-standard and blocks typing during agent responses.

These issues manifest as: duplicate messages on rapid Enter, permanently blocked
input area, "Queued" badges that never clear, agent responses lost on browser
reload, and no recovery from WebSocket disconnection.

## What Changes

### Frontend

- **New `useProposalChat` hook** replaces `useProposalWebSocket` + chat-related
  state in `useAppStore`. Follows Vercel AI SDK `useChat` pattern:
  - Single `messages: Message[]` as sole source of truth (streaming content is
    updated in-place on the last message, not in a separate store).
  - Single `status: 'ready' | 'submitted' | 'streaming' | 'error'` state machine.
  - Encapsulates WebSocket connection, reconnection, and message routing internally.
  - Returns `{ messages, status, sendMessage, stop, error, activeElicitation }`.
- **Remove from `useAppStore`**: `chatMessagesBySessionId`, `streamingContent`,
  `activeTurnBySessionId`, `isAgentResponding`, and all related actions/reducers
  (`START_ASSISTANT_TURN`, `APPEND_STREAMING_CHUNK`, `COMPLETE_ASSISTANT_TURN`,
  `FAIL_ASSISTANT_TURN`, `UPDATE_TOOL_CALL`, `UPDATE_TOOL_CALL_STATUS`,
  `APPEND_CHAT_MESSAGE`, `UPSERT_SERVER_USER_MESSAGE`,
  `UPDATE_CHAT_MESSAGE_SEND_STATUS`, `SET_AGENT_RESPONDING`).
  `activeElicitation` moves into the hook.
- **`ProposalChat` props reduced** from 17+ callback props to ~6
  (`projectId`, `sessionId`, `onBack`, `onMerge`, `onClose`, `onClickChange`).
- **`ChatInput`**: `disabled` controls only the send button; textarea is always
  editable (placeholder text changes to indicate status).
- **Proposal WebSocket reconnection**: exponential backoff matching the main
  `wsClient.ts` pattern (1s → 30s, max 10 retries).
- **Double-send guard in `ChatInput`**: clear input value synchronously before
  calling `onSend` to prevent React batching race on rapid Enter.

### Backend (Rust)

- **`ProposalWsClientMessage::Prompt`**: add optional `client_message_id: Option<String>` field.
- **`ProposalWsServerMessage::UserMessage`**: add optional `client_message_id: Option<String>` field.
- When `client_message_id` is present in the prompt, echo it back in the
  `user_message` response so the frontend can correlate optimistic messages.
- **Backward compatible**: field is optional with `#[serde(default)]`, existing
  clients without the field continue to work.

## Impact

- Affected specs: `proposal-session-ui`, `proposal-ws-streaming`
- Affected frontend files: `useProposalWebSocket.ts` (replaced),
  `useAppStore.ts`, `ProposalChat.tsx`, `ChatInput.tsx`, `ChatMessageList.tsx`,
  `App.tsx`
- Affected backend files: `src/server/api.rs`
  (`ProposalWsClientMessage`, `ProposalWsServerMessage`, `proposal_session_ws`)
- Affected tests: `dashboard/src/hooks/useProposalWebSocket.test.ts`,
  `dashboard/src/store/useAppStore.test.ts`,
  `dashboard/src/components/__tests__/ChatInput.test.tsx`,
  `dashboard/src/components/__tests__/ProposalChat.test.tsx`,
  `tests/e2e_proposal_session.rs`

## Acceptance Criteria

1. Enter double-tap does NOT create duplicate messages.
2. Input textarea is never permanently blocked; typing is always possible.
3. "Queued (will send on reconnect)" badge clears when the server confirms the message.
4. Agent responses survive browser reload (hydrated from history endpoint).
5. WebSocket disconnection triggers automatic reconnection with backoff.
6. `ProposalChat` component receives ≤6 props (excluding `children`).
7. No `streamingContent` or `activeTurnBySessionId` in `useAppStore`.
8. All existing dashboard tests pass after migration.
9. e2e proposal session tests pass with the new `client_message_id` field.

## Out of Scope

- Migrating to Vercel AI SDK as an npm dependency (we adopt the pattern only).
- Server-side streaming resume (`resume: true` equivalent).
- Multi-session concurrent streaming (only one session active at a time).
