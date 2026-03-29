## ADDED Requirements

### Requirement: Dashboard Files pane SHALL provide a toggleable shell terminal panel

The server mode dashboard SHALL provide a shell terminal panel below File Content when the Files pane is active. The panel SHALL be hidden by default and SHALL be revealable and hideable without leaving the current file browsing workflow.

#### Scenario: Files pane shows collapsed terminal toggle by default

**Given** the user has opened the dashboard Files pane
**When** the File Content area is rendered
**Then** the UI shows a terminal toggle below File Content
**And** the terminal panel is collapsed by default

#### Scenario: Expanding the terminal reveals an interactive shell

**Given** the user is viewing the dashboard Files pane
**When** the user expands the terminal toggle
**Then** the dashboard opens a visible interactive terminal panel below File Content
**And** the file browsing UI remains available in the same screen

#### Scenario: Collapsing the terminal hides the panel without discarding sessions

**Given** the user has one or more active terminal tabs in the Files pane
**When** the user collapses the terminal panel
**Then** the terminal UI is hidden
**And** existing terminal sessions remain available for later re-expansion

### Requirement: Dashboard shell terminal SHALL support multiple tabs

The dashboard shell terminal SHALL allow the user to create, switch, and close multiple terminal tabs from the same FileViewPanel.

#### Scenario: User creates a second terminal tab

**Given** the terminal panel is expanded with one active terminal tab
**When** the user activates the add-tab control
**Then** the dashboard creates a new terminal session
**And** the new session appears as a separate selectable tab

#### Scenario: User switches between terminal tabs

**Given** the terminal panel has multiple terminal tabs
**When** the user selects a non-active tab
**Then** the dashboard displays that tab's terminal session output and input target

#### Scenario: User closes an existing terminal tab

**Given** the terminal panel has multiple terminal tabs
**When** the user closes one tab
**Then** the dashboard removes that tab from the tab list
**And** the corresponding server-side terminal session is terminated

### Requirement: Dashboard shell terminal SHALL resolve working directory from file browsing context

When creating a terminal session from the Files pane, the dashboard and server SHALL use the current file browsing context to determine the terminal working directory.

#### Scenario: Worktree context uses worktree directory

**Given** the Files pane is browsing a selected worktree
**When** the user creates or reveals a terminal session
**Then** the terminal session starts in that worktree directory

#### Scenario: Change context without worktree uses base repository directory

**Given** the Files pane is browsing a change context without a selected worktree
**When** the user creates or reveals a terminal session
**Then** the terminal session starts in the base repository directory

#### Scenario: Hidden terminal reuses existing session cwd

**Given** a terminal session was created from a file browsing context
**When** the user hides and later re-shows the terminal panel
**Then** the existing session remains attached to its original working directory
