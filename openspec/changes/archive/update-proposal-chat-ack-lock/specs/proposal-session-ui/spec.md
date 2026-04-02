## MODIFIED Requirements

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL gate proposal chat submission only while a just-submitted user message is awaiting its corresponding server `user_message` acknowledgment. The submission lock SHALL begin when the user submits a message and SHALL end immediately when the matching `user_message` acknowledgment is received. While this submission lock is active, the textarea input and send button SHALL be disabled and the typed text SHALL remain visible. On acknowledgment, the Dashboard SHALL clear the textarea input and re-enable submission immediately. Assistant streaming, tool execution, turn completion, and recovery status SHALL NOT keep the input locked after the user message acknowledgment has been received.

#### Scenario: lock-only-until-user-message-ack

**Given**: An active proposal session with non-empty chat input
**When**: The user submits a message
**Then**: The textarea input and send button are disabled only until the corresponding `user_message` acknowledgment is received

#### Scenario: clear-and-unlock-on-user-message-ack

**Given**: A submitted message is awaiting its `user_message` acknowledgment
**When**: The matching `user_message` acknowledgment is received
**Then**: The textarea input is cleared and the send button is re-enabled immediately

#### Scenario: streaming-does-not-extend-send-lock

**Given**: A previously submitted message has already been acknowledged by `user_message`
**When**: Assistant streaming chunks, tool calls, or turn completion events arrive
**Then**: The textarea input remains enabled and the send button remains enabled for the next submission

#### Scenario: prevent-rapid-double-submit-before-ack

**Given**: A submitted message is still awaiting its matching `user_message` acknowledgment
**When**: The user attempts to submit again before the acknowledgment arrives
**Then**: The second submission is blocked until the first acknowledgment is received
