## ADDED Requirements

### Requirement: opencode-server-client

The system SHALL provide an internal Rust client for the OpenCode Server HTTP API, capable of spawning an `opencode serve` process, performing session/message CRUD, and subscribing to real-time events via SSE.

#### Scenario: spawn-and-health-check

**Given**: The `opencode` binary is available on PATH
**When**: `OpencodeServer::spawn` is called with a working directory
**Then**: An `opencode serve --port 0` process starts, the assigned URL is captured, and `GET /global/health` returns `healthy: true`

#### Scenario: create-session-and-send-prompt

**Given**: A running `OpencodeServer` instance
**When**: `create_session` is called followed by `send_prompt_async` with user text
**Then**: The session is created on the OpenCode Server and the prompt is submitted asynchronously (HTTP 204)

#### Scenario: subscribe-to-events

**Given**: A running `OpencodeServer` instance with an active session
**When**: `subscribe_events` is called
**Then**: SSE events of types `message.part.updated` and `session.status` are parsed into typed `OpencodeEvent` variants
