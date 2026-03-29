## Requirements

### Requirement: Terminal Session Metadata

Terminal sessions SHALL store the `project_id` and `root` context used at creation time, and return them in session listing responses.

#### Scenario: Session created with project context includes metadata

- **GIVEN** a server with no active terminal sessions
- **WHEN** a terminal session is created with `project_id: "proj1"` and `root: "worktree:feature-x"`
- **THEN** the session info returned includes `project_id: "proj1"` and `root: "worktree:feature-x"`
- **AND** listing all sessions also includes these fields

## Requirements

### Requirement: Terminal Session Scrollback Buffer

The server SHALL maintain a ring buffer (up to 64KB) of recent PTY output per session and send the buffered content to newly connected WebSocket clients before streaming live output.

#### Scenario: Reconnecting client receives scrollback

- **GIVEN** a terminal session with prior output exceeding 10 lines
- **WHEN** a new WebSocket client connects to that session
- **THEN** the client receives the buffered scrollback content as binary message(s) before any new live output

#### Scenario: Scrollback buffer respects size limit

- **GIVEN** a terminal session that has produced more than 64KB of output
- **WHEN** the scrollback buffer is queried
- **THEN** only the most recent 64KB of output is retained

### Requirement: Terminal Session Restoration on Page Reload

The dashboard frontend SHALL restore terminal tabs for existing backend sessions when the page loads or when the terminal panel mounts.

#### Scenario: Browser reload restores terminal tabs

- **GIVEN** a user with two active terminal sessions for project "proj1" with root "worktree:feature-x"
- **WHEN** the user reloads the browser page
- **THEN** the terminal panel shows two tabs connected to the existing sessions
- **AND** each tab displays the scrollback content from the server

### Requirement: Terminal Tab Filtering by Worktree Context

The dashboard frontend SHALL filter visible terminal tabs by the currently selected worktree context (`project_id` + `root`), keeping non-matching sessions alive in the background.

#### Scenario: Switching worktree filters terminal tabs

- **GIVEN** a user with one terminal session for root "worktree:feature-x" and one for root "worktree:bugfix-y"
- **WHEN** the user clicks worktree "bugfix-y" in the worktrees panel
- **THEN** only the terminal tab for root "worktree:bugfix-y" is visible
- **AND** the session for "worktree:feature-x" remains alive (PTY running, WebSocket connected)

#### Scenario: Switching back to previous worktree shows its terminals

- **GIVEN** a user who previously switched away from root "worktree:feature-x"
- **WHEN** the user clicks worktree "feature-x" in the worktrees panel
- **THEN** the terminal tab for root "worktree:feature-x" is visible again with its session intact
