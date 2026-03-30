## Implementation Tasks

- [ ] 1. Implement smart auto-scroll in `ChatMessageList.tsx`: track scroll position, only auto-scroll when within 100px of bottom (verification: scroll up during streaming, confirm no forced scroll)
- [ ] 2. Add "New messages" pill button in `ChatMessageList.tsx`: show when new content arrives while scrolled up, click scrolls to bottom (verification: visual confirmation + click behavior)
- [ ] 3. Change `ChatInput.tsx` key bindings: Enter sends, Shift+Enter inserts newline (verification: type multi-line with Shift+Enter, send with Enter)
- [ ] 4. Add typing indicator component: animated 3-dot animation shown when `isAgentResponding` is true and no streaming content exists yet (verification: send prompt, observe dots before first chunk)
- [ ] 5. Integrate typing indicator in `ChatMessageList.tsx` below last message when agent is responding (verification: dots appear after send, disappear when streaming starts)
- [ ] 6. Enhance empty state in `ChatMessageList.tsx`: add Bot icon, descriptive text, and 2-3 example prompt suggestions (verification: open new session, see enhanced empty state)
- [ ] 7. Update existing tests in `ProposalChat.test.tsx` for new key binding behavior (verification: `npm run test` passes in dashboard/)
- [ ] 8. Add test for smart scroll behavior: verify no auto-scroll when scrolled up (verification: `npm run test` passes)

## Future Work

- Mobile-specific optimizations for the chat scroll behavior
- User-configurable key binding preference
