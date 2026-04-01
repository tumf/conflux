## MODIFIED Requirements

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model chat turn lifecycle as a single status state machine with values `ready`, `submitted`, `streaming`, and `error`, plus an explicit reconnect recovery state for transport interruptions during an active turn. The status SHALL be managed by a single `useProposalChat` hook that encapsulates WebSocket connection, message state, and turn tracking. The textarea input SHALL always remain editable; only the send button and submission SHALL be gated by send eligibility. When the WebSocket connection closes unexpectedly during an active turn, the status SHALL transition into reconnect recovery rather than immediately treating the turn as failed. The turn SHALL transition to `error` only after reconnect recovery is exhausted or the server indicates the interrupted turn cannot be recovered.

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
**Then**: The status transitions to `ready` without surfacing a false active-turn failure

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

### Requirement: proposal-session-ui-ws-reconnection

The proposal session WebSocket SHALL automatically reconnect with exponential backoff when disconnected. On reconnection, pending messages SHALL be flushed and message history SHALL be replayed from the server. Reconnect recovery SHALL preserve the current turn when possible and SHALL avoid duplicate submission of prompts that were already accepted before the disconnect.

#### Scenario: auto-reconnect-on-disconnect

**Given**: An active proposal session WebSocket that disconnects
**When**: The disconnection is detected
**Then**: Reconnection attempts begin with exponential backoff (1s, 2s, 4s, 8s, 16s, max 30s) up to 10 retries

#### Scenario: flush-pending-on-reconnect

**Given**: Messages were queued while disconnected
**When**: The WebSocket reconnects successfully
**Then**: Queued messages are sent automatically and their status transitions from "pending" to "sent"

#### Scenario: do-not-duplicate-already-submitted-prompt

**Given**: A prompt was already accepted by the server before the WebSocket disconnected during the active turn
**When**: Reconnect recovery flushes pending work
**Then**: The original prompt is not submitted a second time

#### Scenario: queued-badge-clears-on-confirmation

**Given**: A message displaying "Queued (will send on reconnect)"
**When**: The server confirms receipt via `user_message` with matching `client_message_id`
**Then**: The queued indicator is removed and the message displays normally
