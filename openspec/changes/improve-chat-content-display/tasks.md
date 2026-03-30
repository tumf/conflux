## Implementation Tasks

- [x] 1. Add heading rendering (h1-h3) to `renderMarkdownSimple` in `ChatMessageList.tsx` (verification: send agent response with `# Heading`, confirm styled rendering)
- [x] 2. Add unordered list rendering (- / *) to `renderMarkdownSimple` (verification: list items render with bullets and indentation)
- [x] 3. Add ordered list rendering (1. 2. 3.) to `renderMarkdownSimple` (verification: numbered items render correctly)
- [x] 4. Add link rendering `[text](url)` to `renderInlineMarkdown` (verification: links render as clickable `<a target="_blank">`)
- [x] 5. Add horizontal rule rendering (---) to `renderMarkdownSimple` (verification: `---` renders as `<hr>`)
- [x] 6. Add language label display to fenced code blocks (` ```typescript ` shows "typescript" label) (verification: code block with language shows label)
- [x] 7. Add copy button to fenced code blocks using `navigator.clipboard.writeText` (verification: click copy button, paste confirms copied content)
- [x] 8. Add hover copy button to assistant `MessageBubble` that copies full message content (verification: hover assistant message, click copy, paste confirms)
- [x] 9. Add relative timestamp tooltip to `MessageBubble` using `message.timestamp` (verification: hover message, see "X min ago" text)
- [x] 10. Add unit tests for extended markdown rendering: headings, lists, links, horizontal rules (verification: `npm run test` passes in dashboard/)

## Future Work

- Full GFM table rendering
- Syntax highlighting for code blocks (would require external dependency)
- Image/media rendering in messages
