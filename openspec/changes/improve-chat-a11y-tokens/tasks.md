## Implementation Tasks

- [x] 1. Add `role="dialog"`, `aria-modal="true"`, and `aria-labelledby` to `ElicitationDialog.tsx` overlay container (verification: inspect DOM attributes in browser devtools)
- [x] 2. Implement focus trap in `ElicitationDialog.tsx`: on mount, focus first input; Tab cycles within dialog; Shift+Tab cycles backwards (verification: open dialog, Tab through all focusable elements, confirm no escape)
- [x] 3. Add Escape key handler to `ElicitationDialog.tsx` that triggers `onCancel` (verification: open dialog, press Escape, confirm dialog closes)
- [x] 4. Add click-outside handler to `ElicitationDialog.tsx` backdrop that triggers `onCancel` (verification: click backdrop, confirm dialog closes)
- [x] 5. Extend `index.css` `@theme` block if any missing semantic tokens are needed (e.g., `--color-accent-muted` for tool call backgrounds) (verification: review token coverage)
- [x] 6. Migrate `ProposalChat.tsx` from hardcoded hex to semantic Tailwind tokens (verification: visual diff before/after shows no change)
- [x] 7. Migrate `ChatMessageList.tsx` from hardcoded hex to semantic Tailwind tokens (verification: visual diff)
- [x] 8. Migrate `ChatInput.tsx` from hardcoded hex to semantic Tailwind tokens (verification: visual diff)
- [x] 9. Migrate `ToolCallIndicator.tsx` from hardcoded hex to semantic Tailwind tokens (verification: visual diff)
- [x] 10. Migrate `ProposalChangesList.tsx` from hardcoded hex to semantic Tailwind tokens (verification: visual diff)
- [x] 11. Migrate `ElicitationDialog.tsx` from hardcoded hex to semantic Tailwind tokens (verification: visual diff)
- [x] 12. Update/add tests for ElicitationDialog a11y behavior: Escape closes, focus trap works (verification: `npm run test` passes in dashboard/)

## Future Work

- Screen reader testing with VoiceOver/NVDA
- Extend semantic token migration to non-chat components (Header, ProjectsPanel, etc.)
