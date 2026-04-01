## MODIFIED Requirements

### Requirement: proposal-session-websocket

The system shall provide a WebSocket endpoint that proxies ACP or OpenCode session updates between the Dashboard client and the proposal-session backend. The endpoint SHALL support reconnect recovery for interrupted active turns by replaying enough message/state information for the client to determine whether the interrupted turn is still active or has already completed. The endpoint SHALL also provide heartbeat or keepalive behavior so long-running but otherwise idle turns are less likely to be disconnected by intermediaries.

#### Scenario: prompt-response-flow

**Given**: An active proposal session with WebSocket connected
**When**: The client sends `{ "type": "prompt", "text": "Create auth spec" }`
**Then**: The server sends the prompt to the backing session engine, and streams typed WebSocket messages back (`agent_message_chunk`, `tool_call`, `tool_call_update`, `turn_complete`)

#### Scenario: reconnect-replays-completed-turn

**Given**: A proposal session WebSocket disconnects during an active turn and the server-side turn completes before the client reconnects
**When**: The client reconnects to the same proposal session
**Then**: The server replays enough history/state for the client to reconcile the turn as completed without requiring prompt resubmission

#### Scenario: reconnect-replays-in-progress-turn

**Given**: A proposal session WebSocket disconnects during an active turn and the server-side turn is still in progress when the client reconnects
**When**: The client reconnects to the same proposal session
**Then**: The server replays enough history/state for the client to reconcile the turn as still active and continue receiving updates

#### Scenario: websocket-heartbeat-during-long-turn

**Given**: An active proposal session with a long-running turn and no user-visible message chunks for an extended interval
**When**: The connection remains otherwise healthy
**Then**: The server emits heartbeat or keepalive traffic often enough to reduce idle timeout disconnect risk

#### Scenario: cancel-relay

**Given**: An active proposal session with an ongoing prompt turn
**When**: The client sends `{ "type": "cancel" }`
**Then**: The server cancels the backing turn and the turn ends with stop_reason `cancelled`
