---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/proposal-session-ui/spec.md
  - openspec/specs/dashboard-api/spec.md
  - dashboard/src/hooks/useProposalChat.ts
  - dashboard/src/components/ChatMessageList.tsx
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/components/ChatInput.tsx
  - src/server/api.rs
---

# Change: Fix 3 post-redesign bugs in proposal chat

**Change Type**: implementation

## Why

After the `redesign-proposal-chat-state` change was implemented and archived, three bugs remain:

1. **"Agent is Thinking" indicator stays forever**: `ChatMessageList` still uses the legacy `streamingContent` prop to decide when to hide the typing indicator. Since `ProposalChat` passes `streamingContent={{}}` (always empty), `streamingIds` is always `[]`, so the typing indicator is shown whenever `isAgentResponding` is true — even after streaming content has arrived in `messages[]`.

2. **Agent responses disappear on browser reload**: The REST endpoint `GET /projects/{id}/proposal-sessions/{sid}/messages` is not registered in the server routing table (`api.rs` L3877-3909). `listProposalSessionMessages()` gets a 404. Additionally, `useProposalChat` has a race: `setMessages([])` on line 286 can overwrite messages that arrived via WebSocket replay.

3. **Send button stays disabled after sending**: `handleServerMessage` does not call `transitionStatus('streaming')` on `tool_call` events. When the agent uses tools without emitting `agent_message_chunk`, status stays `submitted` until `turn_complete`. Since `ChatInput` disables the send button when `status !== 'ready'`, it remains disabled throughout.

## What Changes

### Frontend

- **`ChatMessageList`**: Remove `streamingContent` prop entirely. Derive typing indicator visibility from `messages[]` — show it when `isAgentResponding=true` AND the last message in the array is NOT `role: assistant`.
- **`ProposalChat`**: Remove `streamingContent={{}}` prop pass.
- **`useProposalChat`**: Transition status to `streaming` on `tool_call` events. Fix REST/WS race by loading history before connecting WebSocket, and not resetting messages after WS replay has started.
- **`ChatInput`**: No change needed (already correct, gated by `status !== 'ready'`).

### Backend (Rust)

- **`api.rs`**: Add `GET /projects/{id}/proposal-sessions/{session_id}/messages` handler that returns `ProposalSessionMessageRecord[]` via `manager.list_messages()`. Register route in the API router.

## Impact

- Affected specs: `proposal-session-ui` (typing indicator, history hydration), `dashboard-api` (new endpoint)
- Affected frontend: `ChatMessageList.tsx`, `ProposalChat.tsx`, `useProposalChat.ts`
- Affected backend: `src/server/api.rs`

## Acceptance Criteria

1. Typing indicator disappears as soon as the first `agent_message_chunk` or `tool_call` arrives.
2. Browser reload restores all user AND assistant messages.
3. Send button returns to enabled after `turn_complete`, even when the agent only used tools (no `agent_message_chunk`).
4. `GET /api/v1/projects/{id}/proposal-sessions/{sid}/messages` returns 200 with message array.
5. All existing tests pass.
