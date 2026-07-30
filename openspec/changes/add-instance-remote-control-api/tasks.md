## Implementation Tasks

- [ ] **Task 1: Define typed `/api/v2` resource and command DTOs** including capabilities, instance, snapshot, changes, logs, command envelope/status, event envelope, stable error body, revision, sequence, correlation, and idempotency fields. (verification: unit - serialization/schema cases in `cargo test remote_control_api`; verification-id: remote-control-api-local)
- [ ] **Task 2: Add process-incarnation state and bounded registries** for random 128-bit IDs, monotonic revision/sequence, 1000-entry event/log rings, and 1000-entry command/idempotency records with 24-hour completed-record expiry. (verification: unit - limits, expiry, monotonicity, restart, and collision-independent format cases in `cargo test remote_control_api`; verification-id: remote-control-api-local)
- [ ] **Task 3: Mount read resources and coherent full snapshots** for health, capabilities, instance, state, changes, and logs while preserving reducer-derived statuses and cache suppression. (verification: integration - router requests against live in-memory state in `cargo test remote_control_api`; verification-id: remote-control-api-local)
- [ ] **Task 4: Implement `POST /api/v2/commands` and status lookup** with schema validation, lifecycle/revision checks, required idempotency for side effects, canonical replay/conflict behavior, synchronous `200`, asynchronous `202`, and delegation to the shared operator command service. (verification: integration - real service/test-double adapter cases including no-op and failures in `cargo test remote_control_api`; verification-id: remote-control-api-local)
- [ ] **Task 5: Implement SSE replay and v2 WebSocket streaming** with authenticated handshakes, ordered event envelopes, resume sequence handling, explicit gap signaling, and full-state resync guidance. (verification: integration - connected client tests cover replay, live delivery, gap, disconnect, and unauthorized handshake in `cargo test remote_control_api`; verification-id: remote-control-api-local)
- [ ] **Task 6: Add Web bearer-auth configuration and startup safety** through `WebConfig` plus `--web-auth-token`/`--web-auth-token-env` in every existing web CLI scope, rejecting non-loopback bind without a token and keeping only health public. (verification: unit - parser/config/bind/auth matrix in `cargo test remote_control_api`; verification-id: remote-control-api-local)
- [ ] **Task 7: Apply v2 same-origin CORS and stable HTTP mapping** without weakening or silently changing legacy routes. (verification: integration - Origin preflight, auth, and status mapping requests in `cargo test remote_control_api`; verification-id: remote-control-api-local)
- [ ] **Task 8: Extend generated OpenAPI and compatibility coverage** for all v2 HTTP resources while proving legacy `/api/*`, `/ws`, dashboard, and server `/api/v1` remain mounted with existing behavior. (verification: integration - `cargo test remote_control_api` and `make check-openapi`; verification-id: remote-control-api-local)

## Future Work

- A standalone remote CLI or generated SDK.
- Durable audit export if operational demand requires persistence.
- Worktree resources from `secure-remote-worktree-operations`.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-instance-remote-control-api --archive-gate`
