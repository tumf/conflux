## ADDED Requirements

### Requirement: Server mode SHALL manage interactive terminal sessions for the dashboard

The server mode API SHALL let the dashboard create, enumerate, attach to, and delete interactive terminal sessions that are scoped to dashboard terminal tabs.

#### Scenario: Client creates a terminal session

**Given** the dashboard requests a new terminal session for a valid project context
**When** the server receives the session creation request
**Then** the server creates a new terminal session identifier
**And** the response includes the session identifier and session metadata needed for attachment

#### Scenario: Client lists existing terminal sessions

**Given** one or more dashboard terminal sessions exist for the current client workflow
**When** the dashboard requests the terminal session list
**Then** the server returns the active terminal sessions and their identifiers

#### Scenario: Client deletes a terminal session

**Given** a dashboard terminal session exists
**When** the dashboard requests deletion of that session
**Then** the server terminates the associated shell process
**And** the session is removed from the active terminal session list

### Requirement: Server mode SHALL stream interactive terminal I/O over WebSocket

The server mode API SHALL provide a WebSocket attachment for each terminal session so the dashboard can exchange terminal input, output, and terminal-size updates with the shell process.

#### Scenario: Client attaches to an existing terminal session

**Given** a terminal session has been created
**When** the dashboard connects to the session's WebSocket endpoint
**Then** the server attaches the connection to the terminal session
**And** terminal output is streamed to the client

#### Scenario: Client sends terminal input

**Given** the dashboard is attached to a terminal session WebSocket
**When** the user enters shell input in the terminal
**Then** the server forwards that input to the underlying PTY session

#### Scenario: Session cleanup follows detach and delete lifecycle

**Given** a terminal session exists and has been attached by the dashboard
**When** the session is explicitly deleted by the dashboard
**Then** the server stops the associated PTY process
**And** later WebSocket attachment attempts for that session fail
