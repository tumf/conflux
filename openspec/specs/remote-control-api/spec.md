### Requirement: Versioned single-instance remote-control resources

Single-instance web monitoring MUST expose `/api/v2` health, capabilities, instance, state, changes, logs, command, event, and WebSocket resources. It MUST NOT merge this contract into the multi-project server `/api/v1` namespace.

#### Scenario: Client discovers and snapshots one process

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities, instance, and state
**Then**: The client receives supported commands/transports, a process-incarnation ID, and a coherent reducer-derived snapshot

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST accept only `start`, `stop`, `cancel_stop`, `force_stop`, `set_execution_mark`, `set_queue_intent`, `retry_change`, `retry_errors`, `stop_and_dequeue`, and `resolve_merge` until a later spec delta extends the enum. Every command MUST include `expected_revision` and `idempotency_key`. Accepted commands MUST delegate to the shared operator command service; the API MUST NOT maintain an independent workflow state machine.

#### Scenario: Accepted command uses shared behavior

**Given**: A command variant is valid for the current lifecycle state
**When**: The API accepts it
**Then**: The shared operator command service revalidates and executes it
**And**: TUI-equivalent reducer, side-effect, and event semantics apply

#### Scenario: Unknown command type is rejected

**Given**: A command envelope names a type outside the closed enum
**When**: It is submitted
**Then**: The server returns HTTP 422 with `validation_failed`
**And**: No service call occurs

### Requirement: Serialized optimistic revision control

One process-local projection owner MUST serialize command admission, snapshot mutation, `state_revision`, `event_sequence`, event storage, and publication. For each state-affecting input it MUST increment revision exactly once if and only if the snapshot changes and MUST attach that resulting revision to the event. Log-only inputs MUST retain the current revision. Every command MUST supply `expected_revision`; a new stale command MUST fail without service execution.

#### Scenario: State event and snapshot share one revision

**Given**: A state-affecting execution event changes the current snapshot
**When**: The projection owner processes it
**Then**: It increments revision once
**And**: The stored snapshot and published event contain the same resulting revision

#### Scenario: No-op does not advance revision

**Given**: An input produces no snapshot change
**When**: The projection owner processes it
**Then**: `state_revision` is unchanged

#### Scenario: Stale new command is rejected

**Given**: The current state revision is 12
**When**: A new command supplies expected revision 11
**Then**: The server returns HTTP 409 with `stale_revision` and current revision 12
**And**: No command side effect occurs

### Requirement: Structurally idempotent side-effect commands

Within one process incarnation, command identity MUST be the typed tuple `(type, target, params, expected_revision)` after schema defaults are applied. JSON member order and whitespace MUST NOT affect identity. `idempotency_key` and `correlation_id` MUST NOT be identity inputs. Idempotency lookup MUST precede current-revision validation so exact replay returns the original record after state advances; a new key MUST pass revision validation before atomic command/idempotency reservation and service execution.

#### Scenario: Same typed request is replayed

**Given**: An idempotent command completed and state revision later advanced
**When**: The same key and structurally equivalent typed command are submitted again
**Then**: The original command ID and result are returned
**And**: The side effect is not repeated

#### Scenario: Same key with different expected revision conflicts

**Given**: A key is bound to one typed command identity
**When**: The same key is submitted with a different expected revision
**Then**: The server returns HTTP 409 with `idempotency_mismatch`
**And**: No side effect occurs

### Requirement: Bounded fail-closed command and observation history

The process MUST retain the latest 1000 events and 1000 logs. Command and idempotency registries MUST each admit at most 1000 records and MUST reserve corresponding records atomically before executing a side effect. Expired or oldest completed records MAY be evicted; in-progress records MUST NOT be evicted. If capacity cannot be made available without evicting in-progress work, admission MUST return HTTP 503 with `registry_capacity` and MUST NOT execute the command. Completed records MUST expire after 24 hours or process termination.

#### Scenario: In-progress capacity does not permit duplicate execution

**Given**: Registry capacity is occupied by in-progress commands
**When**: A new side-effect command is submitted
**Then**: The server returns `registry_capacity`
**And**: No in-progress record is evicted
**And**: The new side effect is not executed

#### Scenario: Event replay gap requires snapshot

**Given**: A client requests an event sequence older than the retained ring
**When**: It connects to SSE or WebSocket
**Then**: The server signals a replay gap
**And**: The client can recover with `GET /api/v2/state`

### Requirement: Process incarnation and validated correlation

The process MUST generate a new random 128-bit hexadecimal `instance_id` at startup. Events MUST have monotonic process-local `event_sequence` values and include the associated state revision. A caller correlation ID MUST be 1-64 ASCII characters matching `[A-Za-z0-9._:-]+`; it MUST remain an opaque trace label and MUST NOT be used for authorization, resource lookup, uniqueness, or idempotency.

#### Scenario: Restart invalidates replay cursor

**Given**: A client stored an instance ID and event cursor
**When**: The process restarts
**Then**: The instance ID changes
**And**: The old cursor is not treated as valid for the new process

#### Scenario: Invalid correlation ID is rejected

**Given**: A command supplies a correlation ID containing a newline or exceeding 64 characters
**When**: It is submitted
**Then**: The server returns HTTP 422 with `validation_failed`

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

### Requirement: Exact-origin V2 CORS

The `/api/v2` router MUST allow requests without an Origin header, direct same-origin requests, and explicitly configured exact origins. Direct same-origin MUST compare parsed Origin scheme, host, and port with the direct request origin. Wildcards and forwarded-header-derived origin expansion MUST be forbidden. Reverse proxies that change external origin MUST configure an exact allowed origin.

#### Scenario: Foreign origin is denied

**Given**: No matching exact origin is configured
**When**: A browser sends a v2 request from a different origin
**Then**: The server does not grant cross-origin access

#### Scenario: Exact proxy origin is allowed

**Given**: An external reverse-proxy origin is explicitly configured
**When**: A browser sends a request with that exact Origin
**Then**: The server grants cross-origin access without consulting forwarded headers

### Requirement: Stable typed remote-control errors

Every v2 error MUST include `error_code`, sanitized `message`, `correlation_id`, and `current_revision` when applicable. Initial error codes MUST include `unauthorized`, `forbidden`, `not_found`, `stale_revision`, `lifecycle_conflict`, `target_ineligible`, `root_busy`, `idempotency_mismatch`, `registry_capacity`, `validation_failed`, and `internal_error`.

#### Scenario: Client distinguishes stale revision from idempotency mismatch

**Given**: Two requests both map to HTTP 409 for different causes
**When**: One is stale and one reuses an idempotency key with different identity
**Then**: Their error codes are `stale_revision` and `idempotency_mismatch` respectively

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
