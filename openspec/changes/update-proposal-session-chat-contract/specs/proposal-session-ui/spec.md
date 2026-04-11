## MODIFIED Requirements

### Requirement: proposal-session-ui-history-hydration

The Dashboard SHALL restore existing proposal-session messages when reconnecting to or reopening the same session, using WebSocket replay and recovery as the sole history restoration mechanism.

The chat view MUST NOT depend on a separate REST history fetch to populate prior proposal-session messages before rendering replayed WebSocket state.

#### Scenario: reopen-session-restores-history-via-websocket-only

**Given**: A proposal session with existing user and assistant messages persisted by the backend
**When**: The user opens or reopens the chat for that session
**Then**: The WebSocket replay restores the prior messages into the chat list before new live updates are rendered
**And**: no REST history hydration call is required for the session

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL lock proposal-session message submission until the server acknowledges the submitted user message.

While a submitted user message is awaiting server acknowledgment, the textarea input and send button SHALL both be disabled.

If the server acknowledges the user message, the pending user message SHALL transition to `sent` and the input lock SHALL clear.

If delivery cannot be completed after reconnect/recovery attempts, the user message SHALL transition to `failed`, the input lock SHALL clear, and the UI SHALL offer an explicit Retry action that resubmits the message content.

#### Scenario: input-remains-locked-until-user-message-ack

**Given**: A proposal session chat is connected and the user submits a message
**When**: The client is still waiting for the corresponding `user_message` acknowledgment from the server
**Then**: the textarea input is disabled
**And**: the send button is disabled

#### Scenario: ack-clears-submission-lock

**Given**: A submitted user message is pending server acknowledgment
**When**: The server emits the matching `user_message` event for that message
**Then**: the pending user message becomes `sent`
**And**: the submission lock is cleared

#### Scenario: delivery-failure-surfaces-retry

**Given**: A submitted user message could not be delivered successfully after reconnect/recovery attempts
**When**: The client marks the message as failed
**Then**: the message is displayed with `failed` status
**And**: the submission lock is cleared
**And**: the user can trigger Retry to resend the same content
