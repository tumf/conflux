## Implementation Tasks

- [x] 1. Create `ChangesDrawer.tsx` component: slide-in overlay from right with backdrop, `role="dialog"`, `aria-modal="true"`, Escape-to-close (verification: render component, confirm drawer opens/closes)
- [x] 2. Add CSS transition for drawer slide-in (`transform: translateX`) using Tailwind classes (verification: smooth animation on open/close)
- [x] 3. Add toggle button in `ProposalChat.tsx` header with `md:hidden` visibility that opens the drawer (verification: visible on mobile, hidden on desktop)
- [x] 4. Render `ProposalChangesList` inside the drawer, passing existing props (verification: changes list appears in drawer)
- [x] 5. Add backdrop click handler to close the drawer (verification: click outside drawer closes it)
- [x] 6. Add Escape key handler to close the drawer (verification: press Escape closes drawer)
- [x] 7. Add test for drawer open/close behavior (verification: `npm run test` passes in dashboard/)

## Acceptance #1 Failure Follow-up

- [x] Restore `docs/openapi.yaml` to its committed state (`git checkout docs/openapi.yaml`) — the file was emptied outside this change's scope

## Future Work

- Swipe-to-dismiss gesture for mobile
- Remember drawer open/close state across navigation
