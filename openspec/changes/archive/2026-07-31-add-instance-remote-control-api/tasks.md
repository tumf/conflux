## Implementation Tasks

- [x] **Task 1: Define typed `/api/v2` resource, closed-command, and error DTOs** including capabilities, instance, snapshot, changes, logs, command envelope/status, event envelope, stable error codes, revision, sequence, bounded correlation ID, and idempotency fields; reject unknown command variants. (verification: unit - serialization/schema/error-code cases via `cargo test --lib remote_control_api -- --list | grep -q remote_control_api && cargo test --lib remote_control_api`; verification-id: remote-control-api-local)
- [x] **Task 2: Add one process-incarnation projection actor** as the sole owner of random IDs, snapshot, monotonic revision/sequence, and event/log rings; atomically update changed snapshot, revision, event metadata, ring storage, and publication. (verification: unit - changed/no-op/log-only ordering and coherent snapshot concurrency cases via `cargo test --lib remote_control_api -- --list | grep -q remote_control_api && cargo test --lib remote_control_api`; verification-id: remote-control-api-local)
- [x] **Task 3: Add bounded command/idempotency registries** with structural typed identity, replay-before-revision ordering, atomic dual-record reservation, 24-hour completed expiry, completed-only eviction, pinned in-progress records, and fail-closed `registry_capacity`. (verification: unit - JSON-equivalence, revision-inclusive identity, replay, mismatch, expiry, capacity, and no-double-execution cases via `cargo test --lib remote_control_api -- --list | grep -q remote_control_api && cargo test --lib remote_control_api`; verification-id: remote-control-api-local)
- [x] **Task 4: Mount read resources and coherent full snapshots** for health, capabilities, instance, state, changes, and logs while preserving reducer-derived statuses and cache suppression. (verification: integration - router requests against live projection state via `cargo test --lib remote_control_api -- --list | grep -q remote_control_api && cargo test --lib remote_control_api`; verification-id: remote-control-api-local)
- [x] **Task 5: Implement `POST /api/v2/commands` and status lookup** with all-command revision/idempotency requirements, typed error mapping, synchronous `200`, asynchronous/replayed-in-progress `202`, and shared-service target/lifecycle revalidation before effects. (verification: integration - real service/test-double adapter success, stale, replay, no-op, capacity, and failure cases via `cargo test --lib remote_control_api -- --list | grep -q remote_control_api && cargo test --lib remote_control_api`; verification-id: remote-control-api-local)
- [x] **Task 6: Implement authenticated fetch-streamed SSE and non-browser v2 WebSocket** with ordered replay/live delivery, gap signaling, Authorization-header enforcement, rejection of query/subprotocol tokens, and explicit preservation of legacy browser `/ws`. (verification: integration - fetch SSE and WebSocket client tests cover authorized replay/live/gap/disconnect plus unsupported/unauthorized credential transports via `cargo test --lib remote_control_api -- --list | grep -q remote_control_api && cargo test --lib remote_control_api`; verification-id: remote-control-api-local)
- [x] **Task 7: Add Web auth and exact-origin configuration** through `WebConfig` plus mutually exclusive `--web-auth-token`/`--web-auth-token-env` and repeatable `--web-allowed-origin` in every existing web CLI scope; reject unauthenticated non-loopback bind, wildcard origins, malformed origins, and forwarded-header origin expansion. (verification: unit - parser/config/bind/auth/origin matrix via `cargo test --lib remote_control_api -- --list | grep -q remote_control_api && cargo test --lib remote_control_api`; verification-id: remote-control-api-local)
- [x] **Task 8: Extend generated OpenAPI and compatibility coverage** for all v2 HTTP resources while proving legacy `/api/*`, `/ws`, dashboard, and server `/api/v1` remain mounted with existing behavior. (verification: integration - non-empty remote-control suite plus `make check-openapi`; verification-id: remote-control-api-local)

## Implementation Notes

Where the code lives:

- `src/web/remote_control_api/dto.rs` — typed resources, the closed `CommandSpec`
  enum, `CommandRecord`, `EventEnvelope`, and `ErrorCode` (Task 1).
- `src/web/remote_control_api/projection.rs` — the single projection owner
  (snapshot, revision, sequence, event/log rings, command admission) and the
  `OrchestratorStateSnapshot` → `InstanceSnapshot` projection (Task 2).
- `src/web/remote_control_api/registry.rs` — paired command/idempotency
  registries with atomic reservation and pinned in-progress records (Task 3).
- `src/web/remote_control_api/reads.rs` — read resources (Task 4).
- `src/web/remote_control_api/commands.rs` — command submission and lookup;
  `executor.rs` maps the closed command set onto the shared
  `OperatorCommandService` and the existing frontend control channel (Task 5).
- `src/web/remote_control_api/stream.rs` — SSE and WebSocket transports (Task 6).
- `src/web/remote_control_api/auth.rs`, `src/web/mod.rs` (`WebConfig`), and
  `src/cli.rs` — bearer/origin policy, startup validation, and CLI options
  across the root, `run`, and `tui` scopes (Task 7).
- `src/bin/openapi_gen.rs` and `docs/openapi.yaml` — generated schema;
  `tests/compatibility_tests.rs` under the module proves the legacy surface is
  untouched (Task 8).

Two decisions worth recording:

- `OperatorCommandService::resolve_merge` was added so `resolve_merge` stays a
  reducer decision in the shared service. The API owns no workflow state machine.
- Command delegation is late-bound through `RemoteControlRuntime`: the web server
  starts before an orchestration runtime exists, and until one is bound v2
  refuses commands with `lifecycle_conflict` rather than accepting a command it
  cannot act on.

## Future Work

- A standalone remote CLI or generated SDK.
- Durable audit export if operational demand requires persistence.
- One-time browser connection tickets if dashboard migration to protected v2 WebSocket becomes necessary.
- Worktree resources from `secure-remote-worktree-operations`.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-instance-remote-control-api --archive-gate`
