## MODIFIED Requirements

### Requirement: Safe web authentication and binding

The process-scoped v2 server MUST bind a repository-scoped Unix domain socket by default in web-enabled local orchestration. The socket MUST be created with owner-only mode `0600`. Token-free UDS and loopback TCP binding are permitted; non-loopback TCP binding MUST be refused unless a non-empty bearer token is configured. Direct token and token-environment options MUST be mutually exclusive. One configured authentication policy MUST apply to every active listener: `/api/v2/health` remains unauthenticated and every other v2 HTTP, SSE, and WebSocket resource requires `Authorization: Bearer` authentication.

Before UDS bind, a non-socket entry or connectable live socket at the target path MUST be preserved and startup MUST fail. Only an unreachable stale socket entry may be removed. Shutdown MUST remove only the socket entry created by the current process. Tokens in URLs, query parameters, logs, correlation IDs, WebSocket subprotocols, durable browser storage, or Unix endpoint metadata MUST remain forbidden.

#### Scenario: Default UDS is locally accessible without token

**Given**: A web-enabled local orchestration process uses its default socket and no bearer token is configured
**When**: A local client connects through that socket
**Then**: `/api/v2/health` and other v2 resources follow the token-free local policy
**And**: The socket file mode is `0600`

#### Scenario: Configured token protects both transports

**Given**: Bearer authentication and both UDS and TCP listeners are configured
**When**: A client requests a protected v2 resource over either listener without the token
**Then**: Both requests are rejected as unauthorized
**And**: `/api/v2/health` remains available over both listeners

#### Scenario: Unsafe non-loopback startup is rejected

**Given**: Web monitoring is configured on a non-loopback TCP address without a token
**When**: The process starts its listeners
**Then**: Startup fails before any requested listener is published or orchestration begins

#### Scenario: Live socket is preserved

**Given**: The selected Unix path contains a socket accepting connections
**When**: Conflux attempts startup
**Then**: Startup fails without unlinking the socket

#### Scenario: Non-socket target is preserved

**Given**: The selected Unix path contains a regular file or directory
**When**: Conflux attempts startup
**Then**: Startup fails without modifying that entry

#### Scenario: Unreachable stale socket is replaced

**Given**: The selected Unix path contains a socket entry that cannot accept a connection
**When**: Conflux starts the listener
**Then**: It removes the stale socket entry and binds the new listener

#### Scenario: Shutdown does not delete a replacement

**Given**: The process-bound socket path was externally unlinked and replaced after startup
**When**: Conflux shuts down
**Then**: It does not remove the replacement entry
