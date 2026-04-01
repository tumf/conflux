## MODIFIED Requirements

### Requirement: proposal-session-websocket

The system shall provide a WebSocket endpoint that proxies ACP or OpenCode session updates between the Dashboard client and the proposal-session backend. History replay and reconnect recovery SHALL be defined so that the Dashboard can restore proposal-session state idempotently. Replay or restoration traffic SHALL NOT require the client to render the same logical user message, assistant turn, or tool-call sequence more than once.

#### Scenario: reconnect-restoration-is-idempotent

**Given**: A proposal session with persisted message history and a client that reconnects after earlier history has already been loaded
**When**: The server provides replay or restoration data for that same session
**Then**: The replay or restoration contract includes enough identity or source-boundary guarantees for the client to reconcile existing messages without duplicating them in the UI

#### Scenario: replay-identifies-existing-assistant-turn

**Given**: A proposal session with a completed assistant turn containing streamed content and tool calls
**When**: The client reconnects and the server replays that turn
**Then**: The replay data includes stable message or turn identity sufficient for the client to associate the replayed data with the existing logical assistant turn

#### Scenario: restoration-remains-session-scoped

**Given**: Two active proposal sessions exist for the same project
**When**: A client connects or reconnects to the WebSocket for one specific session
**Then**: The server sends only replay or restoration data for that targeted session and does not mix message history from the other session
