## Requirements

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

## Requirements

### Requirement: proposal-session-ui-stable-turn-identity

The Dashboard SHALL preserve each completed assistant turn as a separate message in a proposal session.

#### Scenario: two-sequential-turns

**Given**: A proposal session with one completed assistant response
**When**: The user sends a second prompt and a second assistant response completes
**Then**: Both assistant responses remain visible as distinct messages in chronological order

### Requirement: proposal-session-ui-history-hydration

The Dashboard SHALL restore existing proposal-session messages when reconnecting to or reopening the same session.

#### Scenario: reopen-session-restores-history

**Given**: A proposal session with existing user and assistant messages persisted by the backend
**When**: The user closes and reopens the chat for that same session
**Then**: The prior messages are loaded into the chat list before any new streaming updates are rendered

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model active-turn state explicitly and SHALL enable or disable input based on that state.

#### Scenario: input-disabled-only-during-active-turn

**Given**: An active proposal session and a connected WebSocket
**When**: A prompt is submitted and no completion/error/cancel event has yet been received
**Then**: The input is disabled

#### Scenario: input-reenabled-on-turn-finish

**Given**: An active proposal session with disabled input because a turn is running
**When**: The current turn completes, errors, or is cancelled
**Then**: The input is enabled unless an elicitation dialog remains active or the connection is disconnected


### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model active-turn state explicitly and SHALL enable or disable input based on that state. When the WebSocket connection closes unexpectedly during an active turn, the turn SHALL be treated as failed and the input SHALL be re-enabled.

#### Scenario: input-disabled-only-during-active-turn

**Given**: An active proposal session and a connected WebSocket
**When**: A prompt is submitted and no completion/error/cancel event has yet been received
**Then**: The input is disabled

#### Scenario: input-reenabled-on-turn-finish

**Given**: An active proposal session with disabled input because a turn is running
**When**: The current turn completes, errors, or is cancelled
**Then**: The input is enabled unless an elicitation dialog remains active or the connection is disconnected

#### Scenario: input-reenabled-on-ws-disconnect-during-turn

**Given**: An active proposal session with disabled input because a turn is running
**When**: The WebSocket connection closes unexpectedly
**Then**: The active turn is treated as failed, `isAgentResponding` is set to false, and the input is re-enabled (though still disabled due to disconnected state, it will become usable upon reconnection)


### Requirement: proposal-session-ui-chat

The Dashboard shall provide a chat interface for conversing with the ACP agent during a proposal session. Messages SHALL render Markdown including headings (h1-h3), unordered/ordered lists, links, horizontal rules, code blocks (with language labels and copy buttons), bold text, and inline code. Assistant messages SHALL show a copy button on hover. All messages SHALL display a relative timestamp on hover.

#### Scenario: send-and-receive-messages

**Given**: An active proposal session chat view
**When**: The user types a message and submits
**Then**: The message is sent via WebSocket, and agent responses stream into the message list with Markdown rendering

#### Scenario: tool-call-display

**Given**: The agent executes a tool during a prompt turn
**When**: Tool call updates arrive via WebSocket
**Then**: The tool call is displayed inline with its title and status (pending → in_progress → completed)

#### Scenario: render-markdown-headings-and-lists

**Given**: An assistant message containing Markdown headings and lists
**When**: The message is displayed
**Then**: Headings are rendered as styled h1-h3 elements, and lists are rendered with proper bullets/numbers and indentation

#### Scenario: render-markdown-links

**Given**: An assistant message containing `[text](url)` links
**When**: The message is displayed
**Then**: Links are rendered as clickable `<a>` elements that open in a new tab

#### Scenario: code-block-copy-button

**Given**: An assistant message containing a fenced code block
**When**: The message is displayed
**Then**: The code block shows a language label (if specified) and a copy button that copies the code content to clipboard

#### Scenario: message-copy-button

**Given**: An assistant message is displayed
**When**: The user hovers over the message
**Then**: A copy button appears that copies the full message content to clipboard when clicked

#### Scenario: message-timestamp-on-hover

**Given**: Any message is displayed
**When**: The user hovers over the message
**Then**: A relative timestamp (e.g., "2 min ago") is shown
