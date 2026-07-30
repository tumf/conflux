---
change_type: implementation
priority: high
dependencies:
  - unify-operator-command-execution
references:
  - "openspec/CONSTITUTION.md"
  - "openspec/specs/web-monitoring/spec.md"
  - "openspec/specs/frontend-abstraction/spec.md"
  - "openspec/changes/unify-operator-command-execution/proposal.md"
  - "src/web/mod.rs"
  - "src/web/api.rs"
  - "src/web/state.rs"
  - "src/web/websocket.rs"
  - "src/config/types.rs"
  - "src/cli.rs"
  - "src/remote/client.rs"
verifications:
  - id: remote-control-api-local
    requirement: API schema, auth, revision, idempotency, snapshot/event continuity, and shared-command integration are covered by non-empty repository-local tests.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/api.rs
    evidence: non-empty remote_control_api test listing and passing filtered test output
    rerun: cargo test --lib remote_control_api -- --list | grep -q remote_control_api && cargo test --lib remote_control_api
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: add single-instance remote-control API

**Change Type**: implementation

## Problem / Context

The optional web server exposes legacy monitoring and control routes, while server mode separately owns multi-project `/api/v1`. Neither is a stable, safe contract for remotely controlling one running cflx process with optimistic concurrency, command tracking, replayable events, or a complete capability description.

This change consumes the shared operator command service from `unify-operator-command-execution`. It adds a namespaced `/api/v2` contract without replacing legacy `/api/*`, `/ws`, dashboard behavior, or multi-project `/api/v1`.

## Proposed Solution

Add a single-process `/api/v2` resource API:

- `GET /api/v2/health`
- `GET /api/v2/capabilities`
- `GET /api/v2/instance`
- `GET /api/v2/state`
- `GET /api/v2/changes`
- `GET /api/v2/changes/{change_id}`
- `GET /api/v2/logs`
- `POST /api/v2/commands`
- `GET /api/v2/commands/{command_id}`
- `GET /api/v2/events` using Server-Sent Events
- `/api/v2/ws` using WebSocket

The initial closed command set is `start`, `stop`, `cancel_stop`, `force_stop`, `set_execution_mark`, `set_queue_intent`, `retry_change`, `retry_errors`, `stop_and_dequeue`, and `resolve_merge`. Unknown command types fail schema validation. Every command is a side effect and requires `expected_revision` plus `idempotency_key`; `correlation_id` is optional.

The process exposes random 128-bit hexadecimal `instance_id`, `command_id`, and generated correlation IDs. Caller correlation IDs are opaque 1-64 character ASCII values matching `[A-Za-z0-9._:-]+`; they are never authorization, lookup, uniqueness, or idempotency inputs.

One process-local projection actor owns the coherent snapshot, `state_revision`, `event_sequence`, and event/log rings. It serializes command admission and EventSink projection. Under one projection transaction it applies a state-affecting update, increments revision once only when the snapshot changes, allocates the next event sequence, attaches the resulting revision, and appends the event. Observational log events retain the current revision and receive only a new event sequence.

The latest 1000 events and logs are retained. Command and idempotency registries each admit at most 1000 records. Expired or oldest completed records may be evicted, but in-progress records never are. Admission atomically reserves both records before service execution; if capacity cannot be freed without evicting in-progress work, the server returns `503 registry_capacity` and performs no side effect. Completed records expire after 24 hours or process exit. A sequence gap requires full resynchronization through `GET /api/v2/state`.

Idempotency compares the structurally normalized typed tuple `(type, target, params-with-schema-defaults-applied, expected_revision)`. JSON member order and insignificant whitespace are irrelevant; `idempotency_key` and `correlation_id` are excluded. Idempotency lookup occurs before current-revision validation so an exact replay returns its original record even after state advances. A new key is admitted only after its expected revision matches; the shared service still revalidates lifecycle and target eligibility immediately before effects.

Add bearer-token configuration through `WebConfig` and every CLI scope that currently exposes `--web`, `--web-bind`, and `--web-port`, using mutually exclusive `--web-auth-token` or `--web-auth-token-env`. Environment lookup is recommended because literal CLI values may be visible in process inspection. Non-loopback binding without a resolved non-empty token is rejected before listening. `/api/v2/health` remains unauthenticated; all other v2 paths require `Authorization: Bearer` whenever auth is configured.

Browser consumers use `fetch()` with the Authorization header to consume SSE response streaming; native `EventSource` is not a supported authenticated v2 client. V2 WebSocket requires an Authorization header during upgrade and is therefore for non-browser clients; query-string and WebSocket-subprotocol tokens are rejected. The existing browser dashboard remains on the compatible legacy `/ws` path.

V2 allows requests without `Origin`, actual same-origin requests, and optionally configured exact origins. Same-origin compares the parsed Origin scheme/host/port to the server's direct request origin. Reverse proxies that change the externally visible scheme or authority must configure an exact allowed origin; forwarded headers are not trusted for CORS. Wildcards are forbidden. `WebConfig` and the same web-enabled CLI scopes expose repeatable `--web-allowed-origin <origin>` values.

## Acceptance Criteria

1. A client can discover capabilities and instance incarnation, fetch a coherent full snapshot, submit a closed-set command, query command status, and consume ordered SSE or WebSocket events through the documented client-specific authentication transport.
2. API commands use the shared operator command service; v2 contains no independent workflow state machine.
3. Every command requires `expected_revision`; stale new commands fail before service invocation, while exact idempotent replay returns the original command record.
4. Idempotency is based on normalized typed intent, reserves records before effects, never evicts in-progress work, and never repeats a side effect after capacity pressure.
5. Snapshot revision, event sequence, snapshot mutation, and event append are serialized through one projection owner; clients detect gaps and recover through a full snapshot.
6. Non-loopback startup without bearer authentication fails; health remains public; protected HTTP, fetch-streamed SSE, and non-browser WebSocket access enforce bearer authentication without URL/subprotocol token leakage.
7. V2 CORS is same-origin by default, supports only explicit exact additional origins, and never trusts wildcard or forwarded-header origin expansion.
8. Structured errors include a stable `error_code`, message, correlation ID, and current revision when applicable.
9. Existing legacy monitoring routes, legacy `/ws`, dashboard, and multi-project `/api/v1` remain compatible.

## Explicit Completion Conditions

- Typed v2 request/response DTOs and generated OpenAPI coverage exist for all listed resources and the closed command set.
- The v2 router is mounted only in single-instance web monitoring and is not merged into server-mode project API routing.
- Repository-local tests cover zero-match-resistant verification, health, protected endpoints, loopback/no-auth, non-loopback startup rejection, browser fetch SSE, rejected native EventSource assumptions, authorized/unauthorized WebSocket upgrade, query/subprotocol token rejection, revision conflict, typed error codes, structural idempotency, replay ordering, in-progress capacity pressure, atomic projection, event replay/gap, bounded logs, exact-origin CORS, and legacy compatibility.
- CLI parser/config tests cover mutually exclusive token sources, literal-token warning/documentation, repeatable exact origins, and every current web-enabled command scope.
- The verification first proves at least one matching test exists, then runs filtered tests; `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, generated OpenAPI checks, and strict OpenSpec validation also pass.

## Dependencies

Consumes the base-integrated shared command types/service and lifecycle outcomes from `unify-operator-command-execution`; implementing v2 before that output would require duplicating frontend behavior.

## Out of Scope

- Multi-project server API changes.
- Durable command/event/idempotency storage across process restart.
- Dashboard migration to v2 or a new external client application.
- Browser-native authenticated WebSocket or native EventSource support.
- Worktree mutation endpoints, which are added separately.
- TLS termination, user accounts, OAuth, one-time connection tickets, or distributed leases.
