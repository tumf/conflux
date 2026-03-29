## Implementation Tasks

- [ ] Task 1: Add proposal session types to `dashboard/src/api/types.ts` — `ProposalSession`, `ProposalSessionChange`, `ProposalChatMessage`, `ElicitationRequest`, WebSocket message types (verification: TypeScript compiles without errors)
- [ ] Task 2: Add REST API client functions to `dashboard/src/api/restClient.ts` — `createProposalSession`, `listProposalSessions`, `deleteProposalSession`, `mergeProposalSession`, `listProposalSessionChanges` (verification: TypeScript compiles)
- [ ] Task 3: Create `dashboard/src/hooks/useProposalWebSocket.ts` — WebSocket hook that connects to `/proposal-sessions/{id}/ws`, handles all message types, provides `sendPrompt`, `sendElicitationResponse`, `sendCancel` methods (verification: hook compiles, unit test for message parsing)
- [ ] Task 4: Add proposal session state to `dashboard/src/store/useAppStore.ts` — `proposalSessions`, `activeProposalSessionId`, actions for CRUD operations and message appending (verification: existing store tests still pass, new reducer actions covered)
- [ ] Task 5: Create `dashboard/src/components/ProposalChat.tsx` — main chat container with 2-column layout (chat + changes sidebar), integrates WebSocket hook and renders child components (verification: renders without errors)
- [ ] Task 6: Create `dashboard/src/components/ChatMessageList.tsx` — scrollable message list with Markdown rendering for agent messages, user messages distinguished visually (verification: renders sample messages correctly)
- [ ] Task 7: Create `dashboard/src/components/ChatInput.tsx` — text input with submit button and Ctrl+Enter shortcut, disabled while agent is responding (verification: calls sendPrompt on submit)
- [ ] Task 8: Create `dashboard/src/components/ToolCallIndicator.tsx` — inline display for tool calls with status badge (pending/in_progress/completed/failed) and title (verification: renders all statuses)
- [ ] Task 9: Create `dashboard/src/components/ElicitationDialog.tsx` — modal/inline form that renders ACP restricted JSON Schema: `string` with `oneOf`/`enum` as select, `string` as text input, `boolean` as checkbox, `number`/`integer` as number input; submit sends accept, dismiss sends cancel, explicit decline button (verification: renders test schema, form submission produces correct response)
- [ ] Task 10: Create `dashboard/src/components/ProposalChangesList.tsx` — sidebar listing detected changes from `GET .../changes`, each with change_id and title, click shows proposal.md in existing FileViewPanel (verification: renders change list)
- [ ] Task 11: Create `dashboard/src/components/ProposalActions.tsx` — merge and close session buttons with state-dependent visibility; merge disabled when dirty (verification: button state matches session state)
- [ ] Task 12: Create `dashboard/src/components/CloseSessionDialog.tsx` — confirmation dialog shown when closing dirty session; displays uncommitted file list and force-close button (verification: dialog shows files, force-close calls API with force=true)
- [ ] Task 13: Add "Add Proposal" button to project detail header in `dashboard/src/components/ChangesPanel.tsx` or `dashboard/src/App.tsx` — calls createProposalSession API and navigates to chat view (verification: button visible, creates session on click)
- [ ] Task 14: Add session list/tabs UI for multi-session switching — display active sessions, highlight current, click to switch (verification: can switch between 2 sessions)
- [ ] Task 15: Integrate proposal chat view into `dashboard/src/App.tsx` routing — show chat when activeProposalSessionId is set, back button returns to project view (verification: navigation works)
- [ ] Task 16: Run `cd dashboard && npm run build && npm run test` (verification: build succeeds, tests pass)

## Future Work

- Markdown syntax highlighting for code blocks in chat messages
- File diff view for generated changes
- Drag-and-drop file context into chat input
- Session history/restore UI
