---
change_type: implementation
priority: medium
dependencies: []
references:
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/components/ProposalChangesList.tsx
---

# Change: Add Mobile Changes Drawer for Proposal Chat

**Change Type**: implementation

## Why

The Changes sidebar in the ProposalChat view uses `hidden md:flex` and is completely inaccessible on mobile viewports. Users on mobile devices cannot see which OpenSpec changes the agent has created during a proposal session.

## What Changes

- Add a toggle button in the ProposalChat header (visible only on mobile, `md:hidden`) that opens the Changes list as a slide-in drawer/overlay.
- The drawer slides in from the right, shows the `ProposalChangesList`, and can be dismissed by tapping outside or pressing Escape.
- No external dependency added; implemented with CSS transitions and a backdrop overlay.

## Impact

- Affected specs: `proposal-session-ui`
- Affected code: `ProposalChat.tsx`, possibly a new `ChangesDrawer.tsx` component

## Acceptance Criteria

1. On viewports < 768px (below `md` breakpoint), a button in the chat header toggles the Changes drawer
2. The drawer slides in from the right edge and shows the ProposalChangesList
3. Tapping the backdrop or pressing Escape closes the drawer
4. On desktop (>= 768px), the existing sidebar renders as before; the toggle button is hidden
5. The drawer has proper `role="dialog"` and `aria-modal` attributes
6. All existing tests pass

## Out of Scope

- Full mobile layout redesign
- Bottom sheet pattern (slide-up); using right-side drawer for consistency with desktop sidebar
