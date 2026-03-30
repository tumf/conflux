---
change_type: implementation
priority: high
dependencies: []
references:
  - dashboard/src/index.css
  - dashboard/src/components/ElicitationDialog.tsx
  - dashboard/src/components/ChatMessageList.tsx
  - dashboard/src/components/ChatInput.tsx
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/components/ToolCallIndicator.tsx
  - dashboard/src/components/ProposalChangesList.tsx
---

# Change: Improve Chat Accessibility and Migrate to Semantic Color Tokens

**Change Type**: implementation

## Why

1. **ElicitationDialog lacks accessibility**: No `role="dialog"`, no focus trap, no Escape-to-close, no `aria-labelledby`. This fails WCAG 2.2 dialog requirements.
2. **Hardcoded hex colors throughout**: All chat components use raw hex values (`#27272a`, `#6366f1`, etc.) despite `index.css` defining semantic tokens (`--color-border`, `--color-accent`). This makes theming impossible and violates the project's own design token convention.

## What Changes

- **ElicitationDialog a11y**: Add `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, focus trap (Tab cycling), Escape key to cancel, click-outside to cancel.
- **Semantic token migration**: Replace all hardcoded hex values in chat-related components with Tailwind classes using the `@theme` tokens defined in `index.css` (e.g., `border-[#27272a]` → `border-border`, `bg-[#6366f1]` → `bg-accent`).

## Impact

- Affected specs: `proposal-session-ui`
- Affected code: `ElicitationDialog.tsx`, `ChatMessageList.tsx`, `ChatInput.tsx`, `ProposalChat.tsx`, `ToolCallIndicator.tsx`, `ProposalChangesList.tsx`

## Acceptance Criteria

1. ElicitationDialog has `role="dialog"` and `aria-modal="true"`
2. Focus is trapped within the dialog when open
3. Pressing Escape closes (cancels) the dialog
4. No hardcoded hex color values remain in chat-related components (all use Tailwind semantic tokens)
5. Visual appearance is identical before and after token migration
6. All existing tests pass

## Out of Scope

- Chat scroll and input behavior (separate proposal)
- Markdown rendering (separate proposal)
