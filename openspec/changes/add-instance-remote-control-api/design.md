## Context

Single-instance web monitoring already shares execution events with TUI but lacks a versioned remote-control protocol. Server mode `/api/v1` addresses project supervision and is a separate contract. V2 must make one process observable and controllable without making network state authoritative.

## Goals / Non-Goals

### Goals

- Discoverable, typed, versioned single-instance protocol.
- Coherent snapshots plus resumable ordered event transport.
- Shared command behavior with optimistic concurrency and idempotency.
- Safe bind/auth defaults.
- Legacy compatibility.

### Non-Goals

- Cross-process durability or distributed coordination.
- A replacement for server mode.
- Worktree mutations in this change.

## Decisions

### Decision: resources for reads, one command endpoint for mutations

Read models remain resource-oriented. Mutations use one discriminated command envelope so revision, idempotency, correlation, status tracking, and error semantics remain uniform.

### Decision: process incarnation scopes every transient identifier

`instance_id` changes on each process start. `state_revision`, `event_sequence`, command registry, and idempotency registry are valid only within that incarnation. IDs are random 128-bit lowercase hexadecimal strings. Clients compare instance IDs before attempting replay.

### Decision: snapshot and event contracts are complementary

The state snapshot is coherent at one `state_revision`. Every emitted state-affecting event receives the resulting revision and a monotonic sequence. The event ring retains the latest 1000 entries.

```text
connect(instance_id, after_sequence)
  -> sequence retained: replay then live events
  -> sequence too old or instance mismatch: gap signal
  -> GET /api/v2/state
  -> reconnect from snapshot sequence
```

Logs use the same bounded-observation principle but do not become workflow inputs.

### Decision: idempotency binds canonical intent

Side-effect commands require a caller-supplied idempotency key. The registry stores the canonicalized command fingerprint and its command ID/result. Same key and same fingerprint returns the original record. Same key with a different fingerprint returns `409`. In-progress records return their current asynchronous status.

### Decision: revision requirements depend on destructive sensitivity

Every command may supply `expected_revision`. Commands whose safe meaning depends on an exact target state require it, including active stop-and-dequeue and later worktree delete/merge. A mismatch returns `409` with current revision and performs no service call.

### Decision: authentication is startup-enforced

Loopback binding may run without auth for local tooling. Non-loopback binding requires a resolved non-empty bearer token before socket bind. Token values may be given directly or by environment-variable name; configuration rejects ambiguous conflicting values. Health is always public. All other v2 protocols authenticate before subscription or action.

### Decision: v2 CORS is isolated and same-origin

The v2 router does not inherit wildcard CORS. Requests without cross-origin context and matching-origin requests are allowed; unconfigured foreign origins are denied. Existing routes retain compatibility rather than receiving an accidental security change.

## HTTP and Command Outcomes

- `200`: completed synchronously, replayed completed result, or explicit no-op.
- `202`: accepted and still running.
- `401`: missing/invalid authentication.
- `403`: authenticated but forbidden by configured policy, if later policy exists.
- `404`: unknown resource/command/target.
- `409`: stale revision, lifecycle conflict, ineligible target, root busy, or idempotency mismatch.
- `422`: syntactically valid JSON that violates the command schema.
- `500`: sanitized unexpected internal failure with correlation ID.

## Risks / Trade-offs

- In-memory history cannot replay across restarts. Instance IDs make this explicit and snapshots recover safely.
- A single command endpoint is less REST-pure but avoids inconsistent mutation semantics.
- Bearer auth is intentionally minimal. TLS and identity are delegated to local deployment infrastructure; non-loopback still cannot start accidentally without a token.
