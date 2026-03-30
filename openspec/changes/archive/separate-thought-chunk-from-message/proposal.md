---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/server/api.rs
  - src/server/acp_client.rs
  - dashboard/src/api/types.ts
  - dashboard/src/hooks/useProposalWebSocket.ts
  - dashboard/src/hooks/useProposalWebSocket.test.ts
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/store/useAppStore.ts
  - src/server/proposal_session.rs
---

# Change: Separate AgentThoughtChunk from AgentMessageChunk in Proposal WebSocket

**Change Type**: implementation

## Problem/Context

The proposal chat WebSocket relay in `api.rs:3586-3587` treats `AgentThoughtChunk` (internal chain-of-thought reasoning) identically to `AgentMessageChunk` (user-facing response text). Both are serialized as `agent_message_chunk` and sent to the dashboard frontend.

This causes the agent's internal reasoning (e.g. "The user just said 'hi' - a simple greeting. I should respond concisely...") to appear inline with the actual response in the chat UI, making the conversation confusing and noisy.

## Proposed Solution

Send `AgentThoughtChunk` as a distinct WebSocket message type `agent_thought_chunk` so the frontend can differentiate thought content from message content. The server does not suppress thought chunks — it sends both types with distinct `type` tags. Display/hide decisions are left entirely to the UI layer.

### Server changes (`src/server/api.rs`)

1. Add `AgentThoughtChunk { text: String }` variant to `ProposalWsServerMessage` enum (serializes as `agent_thought_chunk` via `rename_all = "snake_case"`)
2. Split the match arm at line 3586-3587: `AgentMessageChunk` → `ProposalWsServerMessage::AgentMessageChunk`, `AgentThoughtChunk` → `ProposalWsServerMessage::AgentThoughtChunk`
3. Both variants still call `append_assistant_chunk` for message history recording

### History recording (`src/server/proposal_session.rs`)

4. Add `is_thought: Option<bool>` field to `ProposalSessionMessageRecord` (serde `skip_serializing_if = "Option::is_none"`)
5. Introduce `append_assistant_thought_chunk` method (or parameterize the existing `append_assistant_chunk`) that sets `is_thought = Some(true)` on thought records

### Frontend types (`dashboard/src/api/types.ts`)

6. Add `'agent_thought_chunk'` to `ProposalWsMessageType`
7. Add `{ type: 'agent_thought_chunk'; text: string; message_id?: string; turn_id?: string }` to `ProposalWsServerMessage` union

### Frontend hook (`dashboard/src/hooks/useProposalWebSocket.ts`)

8. Add `onThoughtChunk` optional callback to `UseProposalWebSocketOptions`
9. Handle `agent_thought_chunk` case in `handleServerMessage` dispatching to `onThoughtChunk`

### Frontend store / chat (`dashboard/src/components/ProposalChat.tsx`, `dashboard/src/store/useAppStore.ts`)

10. Wire `onThoughtChunk` through `ProposalChat` to store — exact display behavior is out of scope for this change; the default can be to ignore thought chunks (no-op callback) or pass them through with an `isThought` flag

## Acceptance Criteria

- `AgentThoughtChunk` ACP events arrive at the frontend as `{ type: "agent_thought_chunk", text: "..." }`, distinct from `agent_message_chunk`
- `AgentMessageChunk` ACP events continue arriving as `{ type: "agent_message_chunk", text: "..." }` (no regression)
- Message history records include `is_thought` field when the chunk is a thought
- Existing tests pass; new tests cover the split dispatch
- Frontend compiles without type errors

## Out of Scope

- UI rendering decisions for thought chunks (hide, toggle, collapse, etc.) — deferred to a separate UI change
- Changes to the ACP protocol or `acp_client.rs` event definitions (already correctly emits distinct event types)
