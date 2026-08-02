---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - src/web/remote_control_api/dto.rs
  - src/web/remote_control_api/projection.rs
  - src/web/remote_control_api/reads.rs
  - src/tui/state.rs
verifications:
  - id: authoritative-snapshot-tests
    requirement: The v2 snapshot restores every server-authoritative operator decision field without event-history or browser inference
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Rust remote-control projection and API test output
    rerun: cargo test --features web-monitoring remote_control_api
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Expose an authoritative operator snapshot

**Change Type**: implementation

## Problem / Context

`/api/v2/state` exposes canonical display status and progress, but it does not expose enough process-local operator state to reconstruct the TUI after refresh or replay loss. Execution marks have a write command but no readback; queue intent is collapsed into display status; NEW attention, blocker/error details, parallel eligibility, timing, and change-to-worktree relation are absent. A remote frontend would have to infer authority from logs or retained events, violating the frontend boundary and the constitutional rule that observability output cannot control workflow.

## Proposed Solution

Extend the coherent v2 snapshot and change resources with the process-local, non-durable fields required for operator decisions: execution mark, queue intent, attention state, blocker kind/detail, error detail, retry/resolve/kill eligibility and blocked reasons, per-change parallel eligibility, timing, latest structured activity, and worktree relation. Publish the fields at one `state_revision`, restore them after replay gaps, and reset ephemeral intent on process restart without changing workspace-derived routing.

## Acceptance Criteria

1. One `GET /api/v2/state` response contains every server-authoritative field required to render change state and valid operator actions without consulting prior events or parsing logs.
2. Execution mark and queue intent are distinct fields and read back at the revision produced by their mutations.
3. Dependency blocking, external blocking, stalled holds, change-local errors, and fatal process errors remain distinguishable with sanitized details.
4. Parallel eligibility and action eligibility include stable machine-readable reasons rather than requiring Git or lifecycle inference in the client.
5. Active timing, latest structured activity, and change-to-worktree relation survive reconnect and replay-gap resnapshot.
6. All added intent and attention fields remain process-local and non-durable; restart recomputes workflow from workspace/Git evidence and clears ephemeral marks as required by the constitution.

## Explicit Completion Conditions

- v2 DTOs, projection, reads, OpenAPI schemas, and fixtures carry the added fields coherently.
- Projection tests cover every canonical status, blocker class, action-eligibility outcome, empty value, mutation readback, replay-gap resnapshot, and process restart.
- No frontend adapter needs to derive queue intent, blocker class, eligibility, latest activity, or worktree relation from strings, paths, logs, or event history.
- `cargo test --features web-monitoring remote_control_api` passes.

## Out of Scope

- Durable execution marks or UI state outside the workspace.
- New lifecycle commands or parallel-mode mutation.
- Browser UI implementation.
- Exposing absolute worktree paths, tokens, or arbitrary filesystem state.
