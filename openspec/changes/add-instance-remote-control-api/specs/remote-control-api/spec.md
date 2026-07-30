## ADDED Requirements

### Requirement: Versioned single-instance remote-control resources

Single-instance web monitoring MUST expose `/api/v2` health, capabilities, instance, state, changes, logs, command, event, and WebSocket resources. It MUST NOT merge this contract into the multi-project server `/api/v1` namespace.

#### Scenario: Client discovers and snapshots one process

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities, instance, and state
**Then**: The client receives supported commands/transports, a process-incarnation ID, and a coherent reducer-derived snapshot

### Requirement: Shared command delegation

`POST /api/v2/commands` MUST validate a typed command envelope and delegate accepted operator actions to the shared operator command service. The API layer MUST NOT maintain an independent workflow state machine.

#### Scenario: Accepted command uses shared behavior

**Given**: A command is valid for the current lifecycle state
**When**: The API accepts it
**Then**: The shared operator command service executes it
**And**: TUI-equivalent reducer, side-effect, and event semantics apply

### Requirement: Optimistic revision control

The process MUST expose a monotonic process-local `state_revision`. Commands MAY include `expected_revision`; lifecycle-sensitive destructive commands MUST require it. A mismatch MUST return conflict with the current revision and MUST NOT execute the command.

#### Scenario: Stale command is rejected

**Given**: The current state revision is 12
**When**: A revision-required command supplies expected revision 11
**Then**: The server returns HTTP 409 with current revision 12
**And**: No command side effect occurs

### Requirement: Idempotent side-effect commands

Side-effect commands MUST require an idempotency key. Within one process incarnation, the server MUST return the original command record for the same key and canonical command content, and MUST reject reuse of the key with different content.

#### Scenario: Same request is replayed

**Given**: An idempotent command completed
**When**: The same key and canonical command are submitted again
**Then**: The original command ID and result are returned
**And**: The side effect is not repeated

### Requirement: Bounded command and observation history

The process MUST retain at most the latest 1000 events, 1000 logs, 1000 command records, and 1000 idempotency records in memory. Completed command and idempotency records MUST expire after 24 hours or process termination, whichever occurs first.

#### Scenario: Event replay gap requires snapshot

**Given**: A client requests an event sequence older than the retained ring
**When**: It connects to SSE or WebSocket
**Then**: The server signals a replay gap
**And**: The client can recover with `GET /api/v2/state`

### Requirement: Process incarnation and ordered events

The process MUST generate a new random 128-bit hexadecimal `instance_id` at startup. Events MUST have monotonic process-local `event_sequence` values and include the associated state revision and correlation information.

#### Scenario: Restart invalidates replay cursor

**Given**: A client stored an instance ID and event cursor
**When**: The process restarts
**Then**: The instance ID changes
**And**: The old cursor is not treated as valid for the new process

### Requirement: Safe web authentication and binding

The web server MUST refuse non-loopback binding unless a non-empty bearer token is configured. `/api/v2/health` MUST remain unauthenticated. When authentication is configured or binding is non-loopback, every other v2 HTTP, SSE, and WebSocket resource MUST require bearer authentication.

#### Scenario: Unsafe non-loopback startup is rejected

**Given**: Web monitoring is configured on a non-loopback address without a token
**When**: The process starts the web server
**Then**: Startup fails before socket binding

#### Scenario: Protected WebSocket rejects missing token

**Given**: Bearer authentication is required
**When**: A client opens `/api/v2/ws` without valid credentials
**Then**: The handshake is rejected
**And**: No event subscription is created

### Requirement: V2 same-origin CORS

The `/api/v2` router MUST use same-origin CORS by default and MUST NOT configure wildcard origin access.

#### Scenario: Foreign origin is denied

**Given**: No foreign origin allowlist is configured
**When**: A browser sends a v2 request from a different origin
**Then**: The server does not grant cross-origin access

### Requirement: Stable remote-control HTTP outcomes

V2 MUST use `200` for completed/no-op commands, `202` for accepted asynchronous commands, `401/403` for authentication/authorization failure, `404` for missing resources, `409` for revision/lifecycle/eligibility/busy conflicts, and `422` for schema validation errors.

#### Scenario: Invalid command parameters return validation error

**Given**: A JSON command envelope has parameters invalid for its command type
**When**: It is submitted
**Then**: The server returns HTTP 422 with a structured error and correlation ID

### Requirement: Legacy web contract compatibility

Adding `/api/v2` MUST preserve existing single-instance `/api/*`, legacy `/ws`, dashboard routes, and multi-project `/api/v1` behavior.

#### Scenario: Legacy monitoring remains available

**Given**: V2 is mounted
**When**: Existing clients call legacy health/state or connect to legacy WebSocket
**Then**: Their existing routes and response behavior remain available
