---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/cli/spec.md
verifications:
  - id: owner-incarnation-local
    requirement: Owner metadata and API share one immutable process birth without weakening identity checks
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/read_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests owner_incarnation -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests owner_incarnation -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: owner-incarnation-startup
    requirement: Real owner startup publishes coherent identity before TUI wiring and renews it on replacement
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test --test run_exit_tests --features web-monitoring,heavy-tests owner_incarnation -- --nocapture
    rerun: cargo test --test run_exit_tests --features web-monitoring,heavy-tests owner_incarnation -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix owner incarnation start time

**Change Type**: implementation

## Problem / Context

At producer base `80956e9a` (`v0.6.327`), `repo_lock::acquire`
(`src/repo_lock.rs:473-481`) samples `Utc::now()` for the diagnostic owner
record, while `Projection::with_registry`
(`src/web/remote_control_api/projection.rs:110-125`) samples it again for
`GET /api/v2/instance`. These describe one process with two different births.
A read-only observation on 2026-10-25 confirmed PID `239366` in both surfaces,
but owner `started_at = 2026-10-25T08:32:24.070291095+00:00` and API
`started_at = 2026-10-25T08:32:24.070871443+00:00`. The downstream CFSRV-1 live
Web acceptance consumer correctly refuses this exact-identity mismatch.

## Proposed Solution

Add one immutable process-lifetime RFC 3339 birth value to the existing,
feature-independent `src/ids.rs`. Both owner metadata construction and API
projection construction copy that exact value. Preserve the existing random
`instance_id`, PID fields, endpoint array schema, OS lock authority, and
best-effort diagnostic metadata writes. Birth is available before listener
binding, not repaired when TUI state is later attached.

This is one coupled producer fix: metadata and API must agree in the same
binary. Consumer endpoint-schema alignment (array versus map) is a separate
server proposal and can be implemented independently. There is no cross-repo
hard dependency or consumer change in this proposal.

## Acceptance Criteria

1. A freshly started owner publishes exactly equal, nonempty `started_at`
   strings and equal PIDs in its owner record and `/api/v2/instance`, including
   the first response before TUI `AppState` / `set_repo_root` wiring.
2. Endpoint publication, repeated reads, state refresh, and later TUI/runtime
   binding never change the birth during that process lifetime. UDS and TCP
   retain the same shared projection.
3. A replacement process samples a fresh birth and generates a new
   `instance_id`; no persisted record supplies either value. PID reuse cannot
   make an old birth acceptable. Timestamp rounding, tolerance, date parsing,
   and PID-only matching are not introduced.
4. Repository-local wire assertions accept the exact pair and reject a wrong
   PID with the same birth and the same PID with a different birth, including
   a one-nanosecond difference. The consumer's existing strict comparison is
   preserved, not changed or bypassed.
5. Metadata remains observability-only. Missing/malformed/unwritable metadata
   cannot grant ownership or change startup policy. Conflict refusal and
   all-listener-success endpoint publication remain unchanged.

## Explicit Completion Conditions

The shared source and both production call sites are wired; the unit/router and
real-child regression cases in `design.md` run with nonzero test counts under
the two declared focused gates. The startup regression must fail against the
base's independent timestamps and against a fix deferred to TUI binding.
`docs/guides/USAGE.md` explains the immutable identity and fresh-owner rollout.
Proposal validation is not implementation evidence; all tasks remain unchecked
until implemented and tested in a separately authorized execution.

## Scope / Preserved Contracts

Implementation is limited to `src/ids.rs`, `src/repo_lock.rs`,
`src/web/remote_control_api/projection.rs`, their focused tests (including
`src/web/remote_control_api/tests/read_tests.rs`, `src/web/listener_tests.rs`,
and `tests/run_exit_tests.rs`),
and the usage documentation. No new dependency, schema, API route, durable
identity store, mutable identity setter, or TUI orchestration redesign is needed.
The timestamp helper remains available in builds without `web-monitoring`.

The tracked `.pre-commit-config.yaml` scopes Rust checks by staged paths and
has no unconditional Rust test gate. The focused verification declarations
above remain explicit; no broad build/test rerun is added as a checkbox.

## Out of Scope / Rollout Handoff

This delivery is proposal-only: no source/test implementation, merge, archive,
release, installation, owner start/stop, or service change is authorized here.
The future implementation does not edit the consumer or loosen custody, mode,
UID, path, permission, authentication, PID, or birth checks.

An eventual rollout must build/release and pin a binary containing this fix,
then explicitly replace the dedicated owner under separate authority. Installing
a binary alone does not change a running process. Existing live owner records
must not be rewritten, normalized, or backfilled to fabricate agreement; old
owners remain fail-closed. Ordinary acquisition by an authorized new owner may
replace stale diagnostic metadata as it does today. No migration of old records
is planned. Consumer endpoint-schema work and a fresh pinned producer owner are
both prerequisites of the separate CFSRV-1 operational acceptance; this proposal
claims neither rollout nor live Web acceptance.
