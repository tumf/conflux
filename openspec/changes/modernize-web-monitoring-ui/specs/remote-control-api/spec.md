## MODIFIED Requirements

### Requirement: Safe web authentication and binding

The web server MUST refuse non-loopback binding unless a non-empty bearer token is configured. Direct token and token-environment options MUST be mutually exclusive. `/api/v2/health` MUST remain unauthenticated. When authentication is configured, every other v2 HTTP, SSE, and WebSocket resource MUST require `Authorization: Bearer` authentication.

Authenticated browser SSE clients and the embedded operator console MUST use `fetch()` response streaming with the Authorization header. Native `EventSource` MUST NOT be claimed as supported for authenticated v2. The v2 WebSocket MUST require the Authorization header during upgrade and is therefore a non-browser client contract. Tokens in URLs, query parameters, logs, correlation IDs, WebSocket subprotocols, or durable browser storage MUST be rejected or forbidden. The embedded browser console MUST use v2 SSE rather than a separate browser WebSocket contract.

#### Scenario: Unsafe non-loopback startup is rejected

**Given**: Web monitoring is configured on a non-loopback address without a token
**When**: The process starts the web server
**Then**: Startup fails before socket binding

#### Scenario: Browser consumes authenticated SSE through fetch

**Given**: Bearer authentication is required
**When**: Browser code requests `/api/v2/events` with `fetch()` and an Authorization header
**Then**: The server authenticates and streams SSE events

#### Scenario: WebSocket query token is rejected

**Given**: Bearer authentication is required
**When**: A client opens `/api/v2/ws` with a token only in its query or subprotocol
**Then**: The handshake is rejected
**And**: No event subscription is created

#### Scenario: Embedded console keeps token out of durable and observable channels

**Given**: The embedded console accepts a bearer token
**When**: It accesses protected resources
**Then**: The token is sent only in the Authorization header
**And**: It is absent from URLs, logs, correlation IDs, and localStorage

### Requirement: Legacy web contract compatibility

The single-instance web server MUST serve the embedded static operator console and the `/api/v2` contract. The legacy single-instance `/api/*` and `/ws` compatibility surface MUST be removed after the console migrates to v2. Removal MUST NOT affect `/api/v2`, OpenAPI resources, or static asset delivery, and requests to removed mutation routes MUST have no side effect.

#### Scenario: Legacy monitoring route is absent

**Given**: The v2 operator console is mounted
**When**: A client requests a legacy single-instance `/api/*` route or `/ws`
**Then**: The server returns not found
**And**: No command, Git, or orchestration side effect occurs

#### Scenario: Versioned console remains available

**Given**: Legacy monitoring compatibility is removed
**When**: A user opens the console and it accesses `/api/v2`
**Then**: Static assets and versioned resources remain available
**And**: The console can monitor and control the single process through v2
