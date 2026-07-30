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
    requirement: API schema, auth, revision, idempotency, snapshot/event continuity, and shared-command integration are covered by repository-local tests.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/api.rs
    evidence: cargo test output for remote_control_api cases
    rerun: cargo test remote_control_api
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

Command envelopes include `type`, `target`, `params`, optional or command-required `expected_revision`, side-effect-required `idempotency_key`, and optional `correlation_id`. The adapter validates the envelope and delegates operator behavior to the shared command service.

The process exposes random 128-bit hexadecimal `instance_id`, `command_id`, and generated correlation IDs. `state_revision` and `event_sequence` are process-local `u64` counters. The latest 1000 events, 1000 logs, 1000 idempotency records, and 1000 command records are retained in memory; completed command/idempotency records expire after 24 hours or process exit. A sequence gap requires full resynchronization through `GET /api/v2/state`.

Add bearer-token configuration through `WebConfig` and every CLI scope that currently exposes `--web`, `--web-bind`, and `--web-port`, using `--web-auth-token` or `--web-auth-token-env`. Non-loopback binding without a token is rejected before listening. `/api/v2/health` remains unauthenticated; when auth is configured, all other v2 HTTP, SSE, and WebSocket paths require bearer auth. V2 defaults to same-origin CORS rather than wildcard CORS.

## Acceptance Criteria

1. A client can discover capabilities and instance incarnation, fetch a coherent full snapshot, submit commands, query command status, and consume ordered SSE or WebSocket events.
2. API commands use the shared operator command service; v2 contains no independent workflow state machine.
3. `expected_revision` prevents stale destructive or lifecycle-sensitive operations, and the response identifies current revision for resync.
4. Reusing an idempotency key with the same canonical command returns the original result/status; reusing it with different content returns conflict without execution.
5. Event and log retention are bounded at 1000 entries; clients can detect sequence gaps and recover through a full state snapshot.
6. Non-loopback startup without bearer authentication fails; health remains public; protected HTTP/SSE/WebSocket access enforces bearer authentication.
7. V2 CORS is same-origin by default and does not use wildcard `Any`.
8. Existing legacy monitoring routes, legacy `/ws`, dashboard, and multi-project `/api/v1` remain compatible.
9. Error status mapping is stable: `200` completed/no-op, `202` accepted asynchronous command, `401/403` auth, `404` missing resource, `409` revision/lifecycle/eligibility/busy conflict, and `422` schema validation.

## Explicit Completion Conditions

- Typed v2 request/response DTOs and generated OpenAPI coverage exist for all listed resources.
- The v2 router is mounted only in single-instance web monitoring and is not merged into server-mode project API routing.
- Repository-local tests cover health, protected endpoints, loopback/no-auth, non-loopback startup rejection, malformed/unauthorized WebSocket and SSE access, revision conflict, idempotency replay/conflict/eviction, async command status, event replay/gap, bounded logs, and legacy route compatibility.
- CLI parser/config tests cover token literal/env precedence and all current web-enabled command scopes.
- `cargo test remote_control_api`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, generated OpenAPI checks, and strict OpenSpec validation pass.

## Dependencies

Consumes the base-integrated shared command types/service and lifecycle outcomes from `unify-operator-command-execution`; implementing v2 before that output would require duplicating frontend behavior.

## Out of Scope

- Multi-project server API changes.
- Durable command/event/idempotency storage across process restart.
- Dashboard redesign or a new external client application.
- Worktree mutation endpoints, which are added separately.
- TLS termination, user accounts, OAuth, or distributed leases.
