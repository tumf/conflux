# Change: Proposal Session Dashboard UI — Chat Interface and Session Management

## Problem / Context

The `add-proposal-session-backend` change adds ACP-backed proposal sessions with REST+WebSocket APIs. The Dashboard currently has no UI for creating, interacting with, or managing these sessions. Users need a chat interface to converse with the AI agent, respond to elicitation requests, view generated changes, and manage session lifecycle (merge/close).

## Dependencies

- `add-proposal-session-backend` (must be implemented first)

## Proposed Solution

Add Dashboard UI components for the proposal session workflow:

1. **"Add Proposal" button** on the project detail view to start a new session
2. **Chat interface** with Markdown message rendering, streaming responses, and tool call status indicators
3. **Elicitation dialog** that renders ACP form-mode JSON Schema as interactive form inputs
4. **Changes sidebar** showing detected OpenSpec changes in the proposal worktree
5. **Session management** — list active sessions, switch between them, merge to base, close with dirty warning confirmation
6. **Multi-session support** — multiple open sessions displayed as tabs or a session list

## Acceptance Criteria

- "Add Proposal" button visible on project detail page; clicking it creates a session and navigates to chat view
- Chat input accepts text and submits via WebSocket
- Agent responses stream into the message list with Markdown rendering
- Tool call progress is displayed inline (pending → in_progress → completed/failed)
- Elicitation requests render as modal/inline forms based on JSON Schema (string/enum → select, boolean → checkbox, number → input)
- User can accept/decline/cancel elicitation
- Changes sidebar lists detected changes with links to proposal.md preview
- Session close shows confirmation dialog when worktree is dirty, listing uncommitted files
- Force-close button available in dirty confirmation dialog
- Merge button available when worktree is clean; disabled/hidden when dirty
- Multiple sessions can be open simultaneously and switched between

## Out of Scope

- Terminal integration within the chat view (users can use existing terminal panel)
- File editing UI (users rely on agent for file operations)
- Diff view for generated changes
