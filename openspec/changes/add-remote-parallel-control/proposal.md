---
change_type: implementation
priority: high
dependencies:
  - expose-authoritative-operator-snapshot
  - unify-remote-operator-commands
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - src/tui/state.rs
  - src/tui/state/selection_logic.rs
  - src/web/remote_control_api/dto.rs
  - src/web/remote_control_api/executor.rs
verifications:
  - id: remote-parallel-tests
    requirement: Remote clients can observe and safely change parallel execution using the same mode and eligibility rules as the TUI
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Parallel eligibility, bulk-mark, command, and API projection test output
    rerun: cargo test --features web-monitoring --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add remote parallel execution control

**Change Type**: implementation

## Problem / Context

The TUI exposes sequential/parallel mode, maximum concurrency, VCS backend, per-change eligibility, bulk marking, and mode-safe toggling. `/api/v2` exposes none of the runtime mode controls and cannot reproduce atomic bulk-mark semantics. Repeating the single-mark command from a client would classify rows across changing revisions and could leave partial queue/mark state.

## Proposed Solution

Expose parallel runtime state and per-change eligibility in the authoritative snapshot. Add revision-fenced parallel toggle and bulk execution-mark commands implemented by the same application services as the TUI. Bulk marking will classify one coherent snapshot, apply one target state, update Running-mode queue intent consistently, clear applicable NEW attention, and return changed IDs plus exclusion reasons.

## Acceptance Criteria

1. The snapshot reports sequential/parallel mode, availability, maximum concurrency, VCS backend, and per-change eligibility with blocked reasons.
2. Parallel toggle is accepted only in Select or Stopped mode and applies the same ineligible-mark cleanup and feedback as the TUI.
3. Start rejects an ineligible marked set atomically; it never starts only a hidden subset.
4. Bulk mark operates on one revision, ignores excluded rows when selecting the target state, updates all eligible rows atomically, and returns stable exclusion summaries.
5. Running-mode bulk operations keep execution marks and queue intent consistent and produce no partial effects on stale revision, validation failure, or idempotent replay.

## Explicit Completion Conditions

- The closed command enum, DTO schemas, executor, capabilities, projection, and OpenAPI include parallel toggle and bulk mark.
- Shared service tests cover Select, Running, Stopped, Error, and Stopping; Git unavailable; committed/uncommitted; active/rejected/final; all-mark, all-unmark, and zero-eligible cases.
- TUI and v2 parity tests prove equivalent target sets, exclusions, state transitions, and no-op/failure results.
- `cargo test --features web-monitoring parallel remote_control_api` passes.

## Out of Scope

- Changing parallel scheduler algorithms or maximum-concurrency configuration sources.
- Making serial mode durable or preferred; parallel remains the primary mode.
- Browser UI implementation.
