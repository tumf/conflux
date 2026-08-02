## MODIFIED Requirements

### Requirement: Versioned single-instance remote-control resources

Single-instance web monitoring MUST expose `/api/v2` health, capabilities, instance, state, changes, logs, command, event, and WebSocket resources. `/api/v2` is the only versioned remote-control namespace; the removed multi-project `/api/v1` namespace MUST NOT be reintroduced.

#### Scenario: Client discovers and snapshots one process

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities, instance, and state
**Then**: The client receives supported commands/transports, a process-incarnation ID, and a coherent reducer-derived snapshot

#### Scenario: Removed multi-project namespace is not served

**Given**: A single cflx process has web monitoring enabled
**When**: A client requests a `/api/v1` resource
**Then**: The request is not served

### Requirement: Legacy web contract compatibility

Adding `/api/v2` MUST preserve existing single-instance `/api/*` and legacy `/ws` behavior for the local monitoring UI. It MUST NOT depend on the removed multi-project `/api/v1` namespace.

#### Scenario: Legacy monitoring remains available

**Given**: V2 is mounted
**When**: Existing clients call legacy health/state or connect to legacy WebSocket
**Then**: Their existing routes and response behavior remain available
