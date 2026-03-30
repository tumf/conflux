---
change_type: implementation
priority: medium
dependencies: []
references:
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/components/ChatInput.tsx
  - dashboard/src/components/ChatMessageList.tsx
  - dashboard/src/hooks/useProposalWebSocket.ts
---

# Change: Add Message Send Retry on WebSocket Disconnect

**Change Type**: implementation

## Why

When the WebSocket connection drops during a conversation, the user's message is silently lost. There is no queuing, no visual feedback of failure, and no retry mechanism. This creates a frustrating experience where the user must manually retype and resend after reconnection.

## What Changes

- **Failed message indicator**: When a send fails (WS disconnected), the user message is displayed with a red error style and a "Retry" button.
- **Retry on click**: Clicking "Retry" re-sends the message via the WebSocket (if reconnected).
- **Pending state**: Messages sent while disconnected are marked as "pending" with a visual indicator (e.g., clock icon), and automatically sent when the connection is restored.

## Impact

- Affected specs: `proposal-session-ui`
- Affected code: `ProposalChat.tsx`, `ChatInput.tsx`, `ChatMessageList.tsx`, `useProposalWebSocket.ts`, `useAppStore.ts` (message state)

## Acceptance Criteria

1. Sending a message while WS is disconnected adds it to the chat with a "pending" visual state
2. When the WS reconnects, pending messages are automatically sent in order
3. If a pending message fails to send after reconnection, it shows a "Failed" state with a "Retry" button
4. Clicking "Retry" attempts to resend the message
5. Successfully sent retry messages transition to normal display
6. All existing tests pass

## Out of Scope

- Offline message persistence (localStorage) across page reloads
- Batching multiple queued messages into a single prompt
