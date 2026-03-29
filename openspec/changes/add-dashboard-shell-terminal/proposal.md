# Change: Add shell terminal panel to dashboard FileViewPanel

## Why
The server mode dashboard currently lets users inspect files in the Files pane, but it does not provide an interactive shell in the same workflow. Users must leave the dashboard to run repository commands, which breaks the file-inspection flow and makes server mode less useful for proposal and worktree review.

## What Changes
- Add an optional shell terminal panel below File Content in the dashboard Files pane
- Keep the terminal hidden by default and expose a toggle to show or hide it
- Support multiple terminal tabs within the panel
- Add server-side terminal session management and interactive PTY streaming over WebSocket
- Resolve terminal working directory from the current file browsing context: worktree path when browsing a worktree, otherwise base repository path for change context

## Acceptance Criteria
- When the Files pane is open, the UI shows a collapsed terminal toggle below File Content
- Expanding the terminal creates or reveals an interactive shell session without leaving the dashboard
- Users can create, switch, and close multiple terminal tabs from the same panel
- Worktree browsing opens terminals in the selected worktree directory
- Change browsing without a worktree opens terminals in the base repository directory
- Collapsing the panel hides the terminal UI without destroying existing sessions

## Out of Scope
- Persisting terminal sessions across page reloads or server restarts
- Sharing a terminal session across multiple browser clients
- Adding terminal support outside the dashboard Files pane

## Impact
- Affected specs: server-mode-dashboard, server-api
- Affected code: dashboard/src/components/FileViewPanel.tsx, new dashboard terminal component(s), dashboard package dependencies, server-side dashboard/web API and WebSocket routing
