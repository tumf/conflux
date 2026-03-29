## MODIFIED Requirements

### Requirement: proposal-session-ui-chat

The Dashboard SHALL provide a chat interface for conversing with the proposal-session agent, preserving distinct turns and restoring prior messages when the same session is reopened.

#### Scenario: send-and-receive-messages

**Given**: An active proposal session chat view
**When**: The user types a message and submits
**Then**: The message is sent to the backend, and agent responses stream into the message list with Markdown rendering

#### Scenario: preserve-multiple-assistant-turns

**Given**: An active proposal session with two sequential prompts in the same session
**When**: The second assistant response completes
**Then**: The first assistant response remains visible as a separate message and is not overwritten by the second response

#### Scenario: restore-chat-history-on-reopen

**Given**: A proposal session with previously exchanged user and assistant messages
**When**: The user closes the chat panel and reopens the same session
**Then**: The prior messages are restored into the chat list before new streaming begins

### Requirement: proposal-session-ui-input-state

The Dashboard SHALL disable the input only while a prompt turn is actively running and SHALL re-enable it when the turn completes or fails.

#### Scenario: disable-during-active-turn

**Given**: An active proposal session chat view
**When**: The user submits a prompt
**Then**: The input is disabled while the turn is in progress

#### Scenario: re-enable-after-completion

**Given**: An active proposal session with a running turn
**When**: The backend emits `turn_complete`
**Then**: The input field and send button become enabled again unless an elicitation dialog is active or the connection is disconnected
