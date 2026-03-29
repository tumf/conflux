## ADDED Requirements

### Requirement: proposal-session-ui-start

The Dashboard shall provide a button to start a new proposal session from the project detail view.

#### Scenario: start-session-from-button

**Given**: A project detail view is displayed in the Dashboard
**When**: The user clicks "Add Proposal"
**Then**: A new proposal session is created via the API, and the chat interface is displayed

### Requirement: proposal-session-ui-chat

The Dashboard shall provide a chat interface for conversing with the ACP agent during a proposal session.

#### Scenario: send-and-receive-messages

**Given**: An active proposal session chat view
**When**: The user types a message and submits
**Then**: The message is sent via WebSocket, and agent responses stream into the message list with Markdown rendering

#### Scenario: tool-call-display

**Given**: The agent executes a tool during a prompt turn
**When**: Tool call updates arrive via WebSocket
**Then**: The tool call is displayed inline with its title and status (pending → in_progress → completed)

### Requirement: proposal-session-ui-elicitation

The Dashboard shall render ACP form-mode elicitation requests as interactive UI forms.

#### Scenario: render-enum-selection

**Given**: An elicitation request with a string property using `oneOf` enum values
**When**: The elicitation is displayed
**Then**: A select/radio input is rendered with the enum options and the user can choose one

#### Scenario: submit-elicitation-response

**Given**: An elicitation form is displayed
**When**: The user fills in the form and clicks submit
**Then**: An `accept` response with the form data is sent via WebSocket

#### Scenario: cancel-elicitation

**Given**: An elicitation form is displayed
**When**: The user dismisses the dialog
**Then**: A `cancel` response is sent via WebSocket

### Requirement: proposal-session-ui-changes

The Dashboard shall display OpenSpec changes detected in the proposal worktree.

#### Scenario: list-detected-changes

**Given**: The agent has generated `openspec/changes/add-auth/proposal.md` in the worktree
**When**: The changes sidebar is displayed
**Then**: The change `add-auth` appears in the list with its title

### Requirement: proposal-session-ui-close

The Dashboard shall warn users about uncommitted changes when closing a proposal session.

#### Scenario: close-dirty-session-warning

**Given**: A proposal session with uncommitted changes
**When**: The user clicks the close button
**Then**: A confirmation dialog is shown listing uncommitted files with a "Force Close" button

#### Scenario: force-close-confirmation

**Given**: The dirty session confirmation dialog is displayed
**When**: The user clicks "Force Close"
**Then**: The session is closed via the API with `force: true`

### Requirement: proposal-session-ui-merge

The Dashboard shall provide a merge button to merge the proposal worktree into the base branch.

#### Scenario: merge-clean-session

**Given**: A proposal session with committed changes and clean worktree
**When**: The user clicks "Merge"
**Then**: The session is merged via the API, the session is closed, and the user returns to the project view

### Requirement: proposal-session-ui-multi-session

The Dashboard shall support multiple simultaneous proposal sessions per project with tab-based switching.

#### Scenario: switch-between-sessions

**Given**: Two active proposal sessions for the same project
**When**: The user clicks on the second session tab
**Then**: The chat view switches to the second session's conversation
