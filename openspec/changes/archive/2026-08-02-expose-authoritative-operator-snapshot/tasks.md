## Implementation Tasks

- [x] Extend the coherent v2 state/change DTOs with execution mark, queue intent, NEW attention, blocker kind/detail, change-local error detail, action eligibility/reasons, parallel eligibility, timing, latest activity, and worktree relation while preserving process-local/non-durable semantics. (verification: unit - `cargo test --features web-monitoring remote_control_api` verifies DTO serialization and restart-default tests prove every field and ephemeral reset behavior; verification-id: authoritative-snapshot-tests)

- [x] Wire reducer, operator intent store, log projection, and worktree observation into one snapshot revision without deriving values from display strings or frontend state. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies projection fixtures prove coherent blocked/stalled/error/active/final/worktree-linked snapshots and no-op revision behavior; verification-id: authoritative-snapshot-tests)

- [x] Publish ordered state updates whenever an in-scope decision field changes and restore the complete state after replay gaps or process-incarnation changes. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies replay/gap tests replace the snapshot and recover marks, queue intent, eligibility, details, timing, activity, and relations without replay inference; verification-id: authoritative-snapshot-tests)

- [x] Add API and OpenAPI schema coverage for absent values, sanitization, secret/path redaction, mutation readback, and all canonical status/blocker/action combinations. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies authenticated state/change route tests and schema assertions fail on omitted or leaked fields; verification-id: authoritative-snapshot-tests)

## Implementation Notes

New wire surface on every projected change: `execution_marked`, `queue_intent`,
`attention`, `blocker` (with machine-readable `kind`), `error_detail`, `actions`
(per-command `allowed` plus a stable `blocked_reason` token), `parallel`,
`timing`, `latest_activity`, and `worktree`. `InstanceSnapshot` gained
`process_error` so a fatal run failure stays distinguishable from a change-local
one. Absent values serialize as explicit `null`s rather than omitted keys, so one
snapshot can clear a field a client already holds.

Action eligibility is derived from the same lifecycle matrix the TUI uses
(`classify_mark_route` / `classify_retry_route` in
`src/orchestration/operator_command.rs`), so a remote controller and a keypress
cannot be offered different actions. `resolve_merge` is deliberately stricter
than the reducer: it is advertised only for `merge wait`, `resolve pending`, and
`archived`, so nothing reported as `allowed` can fail its guards.

Process-local, non-durable state lives in `src/web/operator_facts.rs` (timing,
latest activity, attention, parallel eligibility, worktree relation) and in the
existing `ExecutionMarkStore`. A restart starts empty by construction and routing
is recomputed from the workspace, as `openspec/CONSTITUTION.md` requires.

Two ordering fixes were required for the snapshot to be authoritative:
`WebState::operator_snapshot` now reads reducer-derived status on every
projection rather than only on event paths that set `updated` (an acceptance hold
changes no monitoring field of its own), and `SharedServiceExecutor::execute`
publishes the projection before a command record settles, since execution marks
and queue intent emit no execution event of their own.

Latest activity excludes streaming output, progress ticks, and logs: including
them would advance `state_revision` on every chunk and leave every client's
optimistic-concurrency token permanently stale.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate expose-authoritative-operator-snapshot --archive-gate`

The implementation must also pass `cargo test --features web-monitoring remote_control_api`.

## Future Work

- Browser presentation and interaction remain in the consuming Web project or `modernize-web-monitoring-ui`.

## Acceptance Repair Notes

Attempt 1 repaired `acceptance-openapi-artifact-emptied` by regenerating
`docs/openapi.yaml` from `openapi-gen` (1565 lines, 48450 bytes), restoring the
artifact this change's own `docs/guides/WEBUI.md` and `docs/guides/WEBUI.ja.md`
cite as the complete API reference.

`Makefile` was also changed, and its relationship to that finding is causal: the
`openapi` target used `cargo run ... > docs/openapi.yaml`, so the shell truncated
the checked-in artifact *before* the generator ran and left it at 0 bytes whenever
the build failed — the exact way the apply commit emptied it. The target now
writes to `/tmp/openapi-gen.yaml` and only `mv`s it into `docs/` on success, which
mirrors what `check-openapi` already did and makes the finding non-recurring.
