## Context

The current proposal chat UI has 5 interconnected bugs that all stem from the
same root cause: the state management architecture diverges from industry-standard
AI chat UI patterns. Rather than patching individual symptoms, this change
redesigns the chat state layer to follow the Vercel AI SDK `useChat` pattern.

Key reference implementations studied:
- Vercel AI SDK `useChat` hook (v5): single `messages[]` + `status` state machine
- assistant-ui `ExternalStoreRuntime`: `isRunning` flag → automatic optimistic updates

## Goals / Non-Goals

**Goals**:
- Single source of truth for messages (`messages[]` array in hook)
- Single status state machine (`ready | submitted | streaming | error`)
- Client↔server message ID correlation for optimistic updates
- Automatic WebSocket reconnection with exponential backoff
- Textarea always editable; only send button respects status

**Non-Goals**:
- Adding Vercel AI SDK as a dependency
- Server-side streaming resume
- Changing the ACP backend protocol

## Decisions

### D1: `useProposalChat` hook as the single integration point

All chat state, WebSocket management, and message lifecycle are encapsulated in
one hook. Components only receive `messages`, `status`, `sendMessage`, `stop`,
`error`, and `activeElicitation`.

**Alternatives considered**:
- Keep `useAppStore` with fixes → rejected because the fragmented state is the
  root cause; patching individual reducers would leave structural coupling.
- Use Vercel AI SDK directly → rejected because it assumes HTTP transport with
  specific response format; our WebSocket protocol is custom.

### D2: Status state machine

```
                    sendMessage()
          ready ──────────────────► submitted
            ▲                           │
            │                    first chunk/tool_call
            │                           │
            │                           ▼
            │                      streaming
            │                           │
            │                    turn_complete
            └───────────────────────────┘
            │
            │         error (ws disconnect, send failure)
            └──────── error ◄───────────┘
```

The `status` is derived from internal state, not a separate `isAgentResponding`
boolean. This eliminates the class of bugs where independent flags disagree.

### D3: Message ID round-trip protocol

```
Client → Server:  { "type": "prompt", "content": "...", "client_message_id": "user-xxx" }
Server → Client:  { "type": "user_message", "id": "srv-uuid", "client_message_id": "user-xxx", ... }
```

The hook matches `client_message_id` to replace the optimistic message with the
server-confirmed version. If no match is found (legacy client), the message is
appended as before.

The `client_message_id` field is `Option<String>` with `#[serde(default)]` in
Rust, ensuring backward compatibility with existing WebSocket clients.

### D4: In-place streaming on messages array

During streaming, the last entry in `messages[]` is the assistant message being
built. Chunks are appended to its `content` field directly. On `turn_complete`,
the message is finalized in place. No separate `streamingContent` store.

### D5: WebSocket reconnection

Follows the same pattern as `dashboard/src/api/wsClient.ts`:
- Delays: [1000, 2000, 4000, 8000, 16000] ms, max 30000ms
- Max attempts: 10
- On reconnect: replay pending prompts via existing `flushPendingPrompts`
- On disconnect during active turn: transition status to `error`

### D6: ChatInput double-send prevention

```typescript
const handleSubmit = useCallback(() => {
  const trimmed = value.trim();
  if (!trimmed || status !== 'ready') return;
  setValue('');          // clear synchronously first
  onSend(trimmed);      // then send
}, [value, status, onSend]);
```

The guard is `status !== 'ready'` rather than a separate `disabled` prop. After
`sendMessage()`, status transitions to `submitted`, preventing further sends
until the turn completes.

## Risks / Trade-offs

- **Risk**: Large refactor touching multiple files simultaneously.
  **Mitigation**: The new hook can be developed alongside the old code and
  swapped in atomically. All existing tests must pass after the swap.

- **Risk**: `useAppStore` still holds `activeElicitation` which could be moved
  into the hook, but elicitation state is session-scoped while the store is global.
  **Decision**: Move `activeElicitation` into the hook since it is inherently
  session-scoped.

- **Trade-off**: Removing chat state from the global store means other components
  cannot read chat messages. Currently no component outside `ProposalChat` reads
  chat state, so this is acceptable.

## Open Questions

None — the design follows a well-established pattern.
