## ADDED Requirements

### Requirement: Safe Unix API socket lifecycle

On Linux and macOS web-enabled builds, the local API MUST use a validated pathname Unix domain socket by default. The selected path MUST be absolute, valid UTF-8, within the platform pathname limit, and have an existing canonical parent whose ownership and mode prevent replacement by another effective UID. The final component MUST NOT be a symlink. The socket MUST be created without any group/other writable interval, set and verified as mode `0600` before endpoint publication, and treated as local-only transport rather than a browser URL.

Startup MUST preserve and reject every pre-existing entry at the target path, including reachable and unreachable sockets. A failed connection attempt MUST NOT classify an entry as stale. Startup MUST NOT automatically unlink a presumed stale socket. Cleanup MUST remove the path only when filesystem identity still matches the socket created by the current process, under the documented trusted-same-UID boundary.

#### Scenario: Secure default socket is created

**Given**: A Linux or macOS web-enabled invocation has a trusted Git common directory and no Unix override or opt-out
**When**: listener startup completes
**Then**: `/api/v2` is available at `${GIT_COMMON_DIR}/cflx-api.sock`
**And**: the socket was never group/other writable
**And**: its verified mode is `0600` before endpoint publication

#### Scenario: Untrusted parent is rejected

**Given**: The selected socket parent permits replacement by another effective UID
**When**: startup validates the path
**Then**: startup fails before bind or orchestration side effects
**And**: the error recommends a trusted explicit path or opt-out

#### Scenario: Invalid path is rejected

**Given**: The selected path is relative, non-UTF-8, over the platform limit, has a missing parent, or has a final-component symlink
**When**: startup validates it
**Then**: startup fails without creating or removing a filesystem entry

#### Scenario: Pre-existing socket is preserved

**Given**: The selected path already contains a reachable or unreachable socket
**When**: startup inspects the target
**Then**: startup fails without unlinking the socket
**And**: the error provides manual verified-stale cleanup guidance

#### Scenario: Pre-existing non-socket entry is preserved

**Given**: The selected path already contains a file, directory, or symlink
**When**: startup inspects the target
**Then**: startup fails without modifying the entry

#### Scenario: Shutdown preserves a replacement

**Given**: The process-created socket path was externally unlinked and replaced
**When**: coordinated shutdown runs cleanup
**Then**: cleanup does not remove the replacement entry

#### Scenario: Non-Unix build is TCP-only

**Given**: Conflux is built for a non-Unix platform
**When**: CLI help or local orchestration runs
**Then**: Unix-only flags are not advertised
**And**: retained TCP behavior remains available when web monitoring is compiled

## MODIFIED Requirements

### Requirement: Safe web authentication and binding

The web server MUST refuse non-loopback TCP binding unless a non-empty bearer token is configured. Direct token and token-environment options MUST be mutually exclusive. Selecting a token-environment option whose variable is absent, non-Unicode, or empty MUST fail startup. Token-free loopback TCP and trusted-path UDS are permitted. `/api/v2/health` MUST remain unauthenticated. When authentication is configured, every other v2 HTTP, SSE, and WebSocket resource on every active listener MUST require `Authorization: Bearer` authentication.

Authenticated browser SSE clients and the embedded operator console MUST use `fetch()` response streaming with the Authorization header. Native `EventSource` MUST NOT be claimed as supported for authenticated v2. The v2 WebSocket MUST require the Authorization header during upgrade and is therefore a non-browser client contract. Tokens in URLs, query parameters, logs, correlation IDs, WebSocket subprotocols, endpoint metadata, or durable browser storage MUST be rejected or forbidden. The embedded browser console MUST use v2 SSE rather than a separate browser WebSocket contract.

#### Scenario: Unsafe non-loopback startup is rejected

**Given**: Web monitoring is configured on a non-loopback address without a token
**When**: The process starts the web server
**Then**: Startup fails before socket binding or endpoint publication

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
**And**: It is absent from URLs, logs, correlation IDs, endpoint metadata, and localStorage

#### Scenario: Configured token protects every transport

**Given**: Bearer authentication and UDS plus TCP listeners are configured
**When**: A client requests a protected v2 resource without the token over either listener
**Then**: Both requests are rejected as unauthorized
**And**: `/api/v2/health` remains available over both listeners

#### Scenario: Selected token environment is invalid

**Given**: A token environment variable name is explicitly configured
**And**: the variable is absent, non-Unicode, or empty
**When**: startup resolves authentication
**Then**: startup fails rather than falling back to token-free service
