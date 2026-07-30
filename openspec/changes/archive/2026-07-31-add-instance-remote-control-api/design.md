## Context

Single-instance web monitoring already shares execution events with TUI but lacks a versioned remote-control protocol. Server mode `/api/v1` addresses project supervision and is a separate contract. V2 must make one process observable and controllable without making network state authoritative.

## Goals / Non-Goals

### Goals

- Discoverable, typed, versioned single-instance protocol.
- Coherent snapshots plus resumable ordered event transport.
- Shared command behavior with optimistic concurrency and idempotency.
- Safe bind/auth/CORS defaults with explicit browser and non-browser transports.
- Legacy compatibility.

### Non-Goals

- Cross-process durability or distributed coordination.
- A replacement for server mode.
- Worktree mutations in this change.
- Dashboard migration from legacy WebSocket.

## Decisions

### Decision: resources for reads, one closed command endpoint for mutations

Read models remain resource-oriented. Mutations use one discriminated command envelope so revision, idempotency, correlation, status tracking, and error semantics remain uniform.

The initial command enum is closed:

- `start`
- `stop`
- `cancel_stop`
- `force_stop`
- `set_execution_mark`
- `set_queue_intent`
- `retry_change`
- `retry_errors`
- `stop_and_dequeue`
- `resolve_merge`

Every listed command is side-effecting and requires `expected_revision` and `idempotency_key`. Proposal 3 extends this enum with worktree commands and retains the same requirement.

### Decision: process incarnation scopes every transient identifier

`instance_id` changes on each process start. `state_revision`, `event_sequence`, command registry, and idempotency registry are valid only within that incarnation. Generated IDs are random 128-bit lowercase hexadecimal strings. Clients compare instance IDs before attempting replay.

Caller-provided `correlation_id` values must be 1-64 ASCII characters from `[A-Za-z0-9._:-]`. They are log/trace labels only and are excluded from command identity, authorization, lookup, and uniqueness guarantees.

### Decision: one projection actor serializes snapshots, revision, and events

One process-local actor is the sole owner of the API snapshot, `state_revision`, `event_sequence`, and observation rings. EventSink input and command admission enter this actor in order.

For a state-affecting input, one actor transaction:

1. applies the update to a candidate snapshot;
2. increments `state_revision` once if and only if the snapshot changed;
3. allocates the next `event_sequence`;
4. attaches the resulting revision and correlation metadata;
5. stores the snapshot and event before publication.

A log-only input allocates a sequence and carries the current revision without incrementing it. A full snapshot includes the latest sequence under the same actor ownership.

```text
connect(instance_id, after_sequence)
  -> sequence retained: replay then live events
  -> sequence too old or instance mismatch: gap signal
  -> GET /api/v2/state
  -> reconnect from snapshot sequence
```

### Decision: idempotency binds normalized typed intent

The identity of a command is the structurally normalized typed tuple `(type, target, params, expected_revision)` after schema defaults are applied. JSON key order and whitespace have no meaning. `idempotency_key` and `correlation_id` are excluded.

Admission order is:

1. validate syntax and typed schema;
2. look up the idempotency key;
3. return the original record for equal typed identity, or `idempotency_mismatch` for unequal identity;
4. for a new key, validate expected revision;
5. atomically reserve command and idempotency records;
6. invoke the shared service, which revalidates lifecycle/target immediately before effects.

This allows replay after revision advancement without allowing a new stale command.

### Decision: in-progress commands are never evicted

Event and log rings each retain 1000 latest entries. Command and idempotency registries each have a 1000-record admission limit. Expired or oldest completed records may be removed to admit work. In-progress records are pinned. If no completed record can be removed, admission returns `503 registry_capacity` and performs no service call. The command/idempotency reservation is atomic so one cannot exist without the other.

### Decision: browser SSE and non-browser WebSocket use different supported clients

Protected HTTP and SSE require `Authorization: Bearer`. Browser code consumes SSE with `fetch()` response streaming, which can set this header. Native `EventSource` is unsupported for authenticated v2.

The v2 WebSocket upgrade also requires the Authorization header, so it is a non-browser client contract. Tokens in query parameters or `Sec-WebSocket-Protocol` are rejected. The browser dashboard continues using legacy `/ws`; one-time browser tickets are future work, not implicit behavior.

### Decision: authentication is startup-enforced

Loopback binding may run without auth for local tooling. Non-loopback binding requires a resolved non-empty bearer token before socket bind. `--web-auth-token` and `--web-auth-token-env` are mutually exclusive; environment indirection is recommended. Health is always public. All other v2 protocols authenticate before subscription or action.

### Decision: CORS uses direct origin or an exact allowlist

Requests without `Origin` and direct same-origin requests are allowed. Same-origin compares normalized scheme, host, and port from the Origin header with the direct request origin. Repeatable configured exact origins allow reverse-proxy deployments whose external origin differs. Wildcards and forwarded-header-derived origins are forbidden.

### Decision: error bodies are machine-actionable

Every non-success response uses `{ error_code, message, correlation_id, current_revision? }`. Initial error codes are:

- `unauthorized`
- `forbidden`
- `not_found`
- `stale_revision`
- `lifecycle_conflict`
- `target_ineligible`
- `root_busy`
- `idempotency_mismatch`
- `registry_capacity`
- `validation_failed`
- `internal_error`

HTTP status is transport classification; `error_code` determines client action.

## HTTP and Command Outcomes

- `200`: completed synchronously, replayed completed result, or explicit no-op.
- `202`: accepted and still running, including replay of an in-progress command.
- `401`: `unauthorized`.
- `403`: `forbidden`.
- `404`: `not_found`.
- `409`: stale revision, lifecycle, eligibility, root-busy, or idempotency conflict with a specific error code.
- `422`: `validation_failed`.
- `503`: `registry_capacity`.
- `500`: sanitized `internal_error`.

## Risks / Trade-offs

- In-memory history cannot replay across restarts. Instance IDs make this explicit and snapshots recover safely.
- A single command endpoint is less REST-pure but avoids inconsistent mutation semantics.
- Standard browser WebSocket/EventSource convenience is intentionally not supported on protected v2; this avoids leaking bearer tokens through URLs or subprotocols.
- Exact-origin configuration is required behind proxies that change scheme/authority because forwarded headers are intentionally untrusted.
