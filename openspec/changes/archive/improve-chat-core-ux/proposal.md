---
change_type: implementation
priority: high
dependencies: []
references:
  - dashboard/src/components/ChatInput.tsx
  - dashboard/src/components/ChatMessageList.tsx
  - dashboard/src/components/ProposalChat.tsx
---

# Change: Improve Chat Core UX (Smart Scroll, Enter-to-Send, Typing Indicator, Empty State)

**Change Type**: implementation

## Why

The current chat UI has several fundamental UX issues that hurt daily usability:
1. Auto-scroll fires unconditionally, forcing users back to the bottom even when reading earlier messages
2. Send requires Ctrl/Cmd+Enter instead of the industry-standard Enter (Slack, ChatGPT, etc.)
3. No visual indicator when the agent is thinking before streaming begins
4. Empty state provides minimal guidance for new users

## What Changes

- **Smart auto-scroll**: Only auto-scroll when user is near the bottom (within 100px). Show a "New messages" pill when new content arrives while scrolled up.
- **Enter-to-send**: Enter submits the message; Shift+Enter inserts a newline.
- **Typing indicator**: Show animated dots ("Agent is thinking...") between prompt submission and first streaming chunk or tool call.
- **Enhanced empty state**: Show an icon, descriptive text, and example prompts.

## Impact

- Affected specs: `proposal-session-ui`
- Affected code: `ChatInput.tsx`, `ChatMessageList.tsx`, `ProposalChat.tsx`

## Acceptance Criteria

1. Scrolling up during streaming does NOT auto-scroll; a "↓ New messages" button appears
2. Clicking the button scrolls to bottom and hides it
3. Pressing Enter with text sends the message; Shift+Enter adds a newline
4. Between send and first streaming chunk, a typing indicator with animated dots is visible
5. Empty chat shows icon, description, and at least 2 example prompts
6. All existing tests continue to pass

## Out of Scope

- Markdown rendering improvements (separate proposal)
- Accessibility improvements (separate proposal)
- Mobile Changes drawer access
