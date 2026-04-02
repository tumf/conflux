## MODIFIED Requirements

### Requirement: proposal-session-ui-enter-to-send

The Dashboard chat input SHALL send the message when the user presses Enter (without modifier keys). Shift+Enter SHALL insert a newline.

#### Scenario: enter-sends-message

**Given**: The chat input has text and is enabled
**When**: The user presses Enter
**Then**: The message is sent and the input is cleared

#### Scenario: shift-enter-inserts-newline

**Given**: The chat input has text and is enabled
**When**: The user presses Shift+Enter
**Then**: A newline character is inserted at the cursor position without sending

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model chat turn lifecycle as a single status state machine with values `ready`, `submitted`, `streaming`, recovery, and `error`. The status SHALL be managed by a single `useProposalChat` hook that encapsulates WebSocket connection, message state, and turn tracking. The textarea input SHALL always remain editable; only the send button and submission SHALL be gated by `status === 'ready'`. When the WebSocket connection closes unexpectedly during an active turn, the status SHALL transition into reconnect recovery first and SHALL return to `ready` once replay confirms the interrupted turn already completed.

#### Scenario: status-transitions-on-send

**Given**: An active proposal session with status `ready`
**When**: The user submits a message
**Then**: The status transitions to `submitted`; the send button is disabled; the textarea remains editable

#### Scenario: status-transitions-to-streaming

**Given**: Status is `submitted` after sending a prompt
**When**: The first `agent_message_chunk` or `tool_call` arrives
**Then**: The status transitions to `streaming`

#### Scenario: status-returns-to-ready

**Given**: Status is `streaming` during an active turn
**When**: A `turn_complete` message is received
**Then**: The status transitions to `ready` and the send button is re-enabled

#### Scenario: active-turn-disconnect-enters-recovery

**Given**: Status is `submitted` or `streaming` during an active turn
**When**: The WebSocket connection closes unexpectedly
**Then**: The status transitions to reconnect recovery, the active turn is not yet treated as failed, and automatic reconnection begins

#### Scenario: active-turn-recovery-completes-after-reconnect

**Given**: An active turn was interrupted by an unexpected WebSocket disconnect
**When**: The WebSocket reconnects and replay/history shows that the server-side turn already completed
**Then**: The status transitions to `ready`, the send button is re-enabled, and the user is not required to resend the original prompt

#### Scenario: active-turn-recovery-resumes-streaming

**Given**: An active turn was interrupted by an unexpected WebSocket disconnect
**When**: The WebSocket reconnects and replay/history shows that the turn is still in progress
**Then**: The UI returns to an in-progress state without requiring the user to resend the original prompt

#### Scenario: active-turn-recovery-terminal-failure

**Given**: An active turn was interrupted by an unexpected WebSocket disconnect
**When**: Reconnection exceeds the retry budget or the server indicates the interrupted turn cannot be recovered
**Then**: The status transitions to `error` and the UI surfaces a terminal recovery failure

#### Scenario: textarea-always-editable

**Given**: Any chat status (`ready`, `submitted`, `streaming`, recovery, or `error`)
**When**: The user attempts to type in the textarea
**Then**: The textarea accepts input regardless of status
