## Implementation Tasks

- [ ] 1. Add `sendStatus` field to `ProposalChatMessage` type in `api/types.ts`: `'sent' | 'pending' | 'failed'` (verification: type compiles with no errors)
- [ ] 2. Update `useAppStore.ts` to track pending/failed message state per session (verification: store correctly updates message status)
- [ ] 3. Modify `ProposalChat.tsx` `handleSend` to set `sendStatus: 'pending'` when WS is disconnected, and queue the message (verification: send while disconnected, message appears with pending state)
- [ ] 4. Add pending message queue in `useProposalWebSocket.ts`: on reconnection, flush queued messages in order (verification: disconnect, send message, reconnect, message auto-sends)
- [ ] 5. Update `ChatMessageList.tsx` `MessageBubble` to show pending indicator (clock icon + muted style) for `sendStatus === 'pending'` (verification: visual confirmation of pending state)
- [ ] 6. Update `ChatMessageList.tsx` `MessageBubble` to show failed indicator (red border + "Retry" button) for `sendStatus === 'failed'` (verification: visual confirmation + retry button visible)
- [ ] 7. Implement retry handler: clicking "Retry" re-sends the message and transitions status back to `'pending'` (verification: click retry, message transitions to pending then sent)
- [ ] 8. Mark message as `'failed'` if send attempt errors after reconnection (verification: force error scenario, confirm failed state)
- [ ] 9. Add tests for pending/retry flow: queue while disconnected, auto-send on reconnect, retry on failure (verification: `npm run test` passes in dashboard/)

## Future Work

- Persist pending messages to localStorage for page reload resilience
- Exponential backoff for retry attempts
- Toast notification when queued messages are auto-sent
