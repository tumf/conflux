## MODIFIED Requirements

### Requirement: proposal-session-websocket

The system shall provide a WebSocket endpoint that proxies ACP JSON-RPC messages between the Dashboard client and the ACP agent subprocess, while preserving backend-managed specification-focused prompt guidance for proposal chat.

WebSocket replay and reconnect recovery SHALL be the sole history restoration mechanism for proposal-session chat. REST message-history endpoints MUST NOT be required as the authoritative hydration baseline for initial session load or reconnect.

Replay and recovery payloads SHALL include stable message or turn identity sufficient for the Dashboard to reconcile repeated frames idempotently.

WebSocket-scoped relay tasks MUST terminate with the WebSocket connection so stale tasks cannot continue consuming ACP notifications after disconnect.

#### Scenario: reconnect-restores-history-without-rest-baseline

**Given**: A proposal session has persisted user and assistant messages and the Dashboard opens or reconnects to that session
**When**: The server upgrades the proposal-session WebSocket connection
**Then**: The server replays the existing session history and recovery state over that WebSocket
**And**: the Dashboard does not require a separate REST history baseline to render prior messages

#### Scenario: replay-identifies-existing-assistant-turn

**Given**: A proposal session with a completed assistant turn containing streamed content and tool calls
**When**: The client reconnects and the server replays that turn
**Then**: The replay frames include stable `message_id` and/or `turn_id` values sufficient for the client to associate the replayed data with the existing logical assistant turn without duplication
