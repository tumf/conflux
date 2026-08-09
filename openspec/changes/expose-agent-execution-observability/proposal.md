---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/observability/spec.md
  - src/web/remote_control_api/dto.rs
  - src/web/remote_control_api/reads.rs
  - src/web/remote_control_api/projection.rs
  - src/web/remote_control_api/registry.rs
  - src/web/remote_control_api/executor.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/operator_coordinator.rs
  - tests/openapi_contract_tests.rs
  - https://github.com/tumf/conflux/issues/14
verifications:
  - id: agent-execution-observability-tests
    requirement: "Remote agents can distinguish scheduler availability, active lifecycle work, completed phase boundaries, and phase-aware stop settlement from structured API evidence without parsing display strings, reading Git independently, or accessing log files"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust API, projection, command-settlement, deterministic race, privacy, and OpenAPI contract test output"
    rerun: "cargo test agent_execution_observability"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Expose agent-readable execution observability

**Change Type**: implementation

## Premise / Context

- External agents already operate a live `cflx tui` through the repository-local `/api/v2` Unix socket.
- A real operator flow treated command success plus `display_status=applying` as proof that no Apply commit existed, although Apply had completed before `stop_and_dequeue` settled.
- `/api/v2/state` exposes reducer presentation, timing boundaries, and latest lifecycle activity, while `/api/v2/logs` exposes the bounded structured log ring; clients still have to infer whether work is actually active and correlate the latest line themselves.
- `stop_and_dequeue` currently settles as `OperatorOutcome::Dequeued { change_id }` and the command record only says that the change was cancelled and dequeued.
- Log file paths are host-private and must remain undisclosed over UDS and TCP. Agents must obtain observable log content through authenticated API resources only.
- The Constitution permits process-local observability but forbids logs or out-of-worktree state from becoming workflow-control authority.

## Problem / Context

The current API exposes enough low-level fields to observe orchestration, but not a closed machine-readable answer to the questions an agent must ask before intervening:

- Is a scheduler merely alive, or is lifecycle work currently active?
- Which phase is active, and which phase most recently completed?
- When did that phase and its latest observable activity occur?
- What is the latest structured log line for this change?
- When `stop_and_dequeue` succeeds, which phase was actually cancelled?
- Had Apply already produced its final commit before cancellation settled?
- Did dequeue undo any completed worktree effect?

Display strings and generic command detail cannot safely answer those questions. This ambiguity can cause an agent to create a duplicate manual commit or manually archive or merge while Conflux-owned lifecycle work is still settling.

## Proposed Solution

Add an authenticated `GET /api/v2/execution-status` read resource that coherently joins the current process snapshot with the bounded in-memory structured log ring. It will expose:

- process incarnation, state revision, event sequence, observation time, application mode, scheduler liveness, and a typed `has_active_work` result;
- each relevant change's typed execution state, current phase, last completed phase, iteration, start/completion boundaries, latest lifecycle activity, and latest exact-`change_id` structured log entry;
- UTC RFC 3339 absolute timestamps only, including `observed_at`; no server-generated elapsed duration or “N minutes ago” text;
- `null` for unavailable evidence instead of inferred values.

The resource will not advance `state_revision` merely because time passed or a log line arrived. Its response will read snapshot and retained logs under one projection-owner lock, while `state_revision` continues to describe authoritative state and `event_sequence` identifies the observation cursor.

Extend settled command records with a closed typed `result` payload. For successful `stop_and_dequeue`, record settlement evidence captured after termination confirmation and final lifecycle revalidation:

- `cancelled_phase`;
- `last_completed_phase`;
- whether the managed worktree contains the final Conflux Apply commit and its commit OID when determinable;
- `effects_rolled_back: false`.

The operator detail will state that dequeue does not roll back previously completed worktree effects. Unknown phase or Git evidence remains explicitly unknown. Exact replay returns the original fixed result and never recomputes it from later state.

No log path, workspace path, repository path, file URL, arbitrary file-read parameter, or persistent-log tail endpoint will be exposed. Full retained API logs remain available through `GET /api/v2/logs`, and live log/event observation remains available through the existing event transports.

## Acceptance Criteria

1. An authenticated v2 client can read `GET /api/v2/execution-status` over UDS or TCP and determine separately whether the scheduler is live and whether lifecycle work is active.
2. Process and change execution states use closed typed vocabularies rather than display strings or free-form log parsing.
3. Each observed change reports `current_phase`, `last_completed_phase`, iteration, and available start/completion boundaries from process-local lifecycle facts without making those facts durable workflow authority.
4. Every returned timestamp is an absolute UTC RFC 3339 instant. The API returns no elapsed seconds, age seconds, or localized relative-time text.
5. `observed_at` lets clients render relative time against the server observation instant without relying on the client clock.
6. Each change's `latest_log` is the newest retained sanitized `LogEntry` whose `change_id` exactly matches; a process-level latest log is also returned; absence is `null`.
7. Log-only activity can change `latest_log` and `event_sequence` without advancing `state_revision`.
8. Agents can retrieve the complete retained structured log ring through `GET /api/v2/logs` and live observation transports, subject to existing authentication, bounds, sanitization, and replay rules.
9. No v2 response, schema, command result, event, or capability exposes the persistent log path, workspace path as a log locator, file URL, or arbitrary host-file read facility over either transport.
10. A settled successful `stop_and_dequeue` command returns a typed result containing `cancelled_phase`, `last_completed_phase`, Apply commit presence, optional commit OID, and `effects_rolled_back=false`.
11. Apply commit presence and OID are derived from managed-worktree Git evidence at settlement. Indeterminate evidence is represented as unknown and never guessed from task count, display status, logs, or a commit subject alone.
12. The human-readable stop detail distinguishes cancellation before final Apply commit, cancellation after Apply completion, and later lifecycle cancellation when evidence permits, and always states that prior effects were not rolled back.
13. Exact idempotent replay preserves the original structured result even if later lifecycle or Git state changes.
14. A deterministic regression test proves that when the final Apply commit lands before stop settlement and acceptance is then cancelled, the result reports `cancelled_phase=acceptance`, `last_completed_phase=apply`, the retained Apply commit OID, and no rollback.
15. Tests also cover stopping before the final Apply commit, stopping during archive or resolve, unavailable Git evidence, no matching log, log-only updates, authentication, and path non-disclosure.
16. The generated canonical OpenAPI contract includes the execution-status route, closed execution/phase schemas, absolute-time fields, and typed command result variants.

## Explicit Completion Conditions

- `src/web/remote_control_api/dto.rs` defines the execution-status and typed command-result wire contracts with closed enums, nullable unknown evidence, and RFC 3339 timestamp descriptions.
- `src/web/remote_control_api/projection.rs` provides one bounded coherent read of snapshot and retained logs without changing state revision semantics.
- `src/web/remote_control_api/reads.rs` and router wiring serve the authenticated execution-status resource and select latest logs by exact structured association.
- Shared lifecycle instrumentation records current and completed phase boundaries from typed execution events; no display-string or log parsing supplies authoritative phase values.
- `src/orchestration/operator_command.rs` and `src/orchestration/operator_coordinator.rs` capture phase and managed-worktree Apply commit evidence after confirmed termination and final revalidation.
- `src/web/remote_control_api/registry.rs` stores the typed command result with the settled record, and idempotent replay returns it unchanged.
- Focused tests named with `agent_execution_observability` prove real projection, log correlation, phase transitions, Git evidence, stop races, replay stability, authentication, privacy, and OpenAPI output rather than dummy DTO serialization alone.
- The declared `agent-execution-observability-tests` verification passes.

## Scope Rationale

Execution status and phase-aware stop settlement are one safety contract. The status resource explains what is currently happening, while the command result fixes the evidence at the exact intervention boundary. Shipping either alone would leave the same remote agent able to misread the other side of the transition.

## Out of Scope

- Exposing persistent log file paths over UDS, TCP, OpenAPI, events, or command results.
- Reading, tailing, downloading, or searching arbitrary host files through the API.
- Increasing the existing 1000-entry API log retention ring or changing persistent file-log retention.
- Making logs, API phase facts, command records, or process-local observations workflow-control inputs.
- Returning localized relative-time strings, elapsed counters, or continuously advancing revisions.
- Automatically resuming, archiving, merging, committing, or rolling back work after `stop_and_dequeue`.
- Requiring clients to inspect Git independently before interpreting the stop result.

The Rust hooks in `.pre-commit-config.yaml` are path-scoped and do not run for this proposal-only commit. Requirement-specific focused tests remain explicit implementation evidence; implementation commits remain subject to the Rust hooks when Rust paths are staged.
