---
change_type: implementation
priority: medium
dependencies: []
references:
  - dashboard/src/components/ChatMessageList.tsx
---

# Change: Improve Chat Content Display (Markdown, Copy, Timestamps)

**Change Type**: implementation

## Why

The current `renderMarkdownSimple` only handles code blocks, bold, and inline code. Common agent responses include headings, lists, and links which render as plain text. Additionally, there is no way to copy message content or code blocks, and timestamps are not visible.

## What Changes

- **Markdown rendering**: Extend `renderMarkdownSimple` to support headings (h1-h3), unordered/ordered lists, links, and horizontal rules. Add language label and copy button to fenced code blocks.
- **Message copy**: Add a copy-to-clipboard button on assistant messages (visible on hover).
- **Timestamps**: Show relative timestamp on hover for each message (e.g., "2 min ago").
- **Code block copy**: Add a copy button to the top-right of each code block.

## Impact

- Affected specs: `proposal-session-ui`
- Affected code: `ChatMessageList.tsx`

## Acceptance Criteria

1. Markdown headings (# ## ###) render as styled headings
2. Unordered and ordered lists render correctly with proper indentation
3. Links render as clickable `<a>` elements with `target="_blank"`
4. Fenced code blocks show language label (if specified) and a copy button
5. Hovering an assistant message shows a copy button; clicking copies the full message content
6. Hovering any message shows a relative timestamp
7. All existing tests pass

## Out of Scope

- Full GFM table support
- Image rendering within messages
- External markdown library adoption
