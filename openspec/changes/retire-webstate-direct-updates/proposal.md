---
change_type: implementation
priority: medium
dependencies: []
references:
  - "src/web/state.rs"
verifications:
  - id: focused-tests
    requirement: The refactoring preserves the named behavior and proves the corrected boundary
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/state.rs
    evidence: cargo test web::state tui::command_handlers::cross_adapter_tests --lib && cargo test --test client_cli_tests --test client_completion_sink
    rerun: cargo test web::state tui::command_handlers::cross_adapter_tests --lib && cargo test --test client_cli_tests --test client_completion_sink
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retire the unscoped WebState update API

**Change Type**: implementation

## Problem / Context

Event-driven production projection writes use the dispatcher, while periodic disk refresh legitimately calls `WebState::update_with_mode` from `refresh_from_disk_at`. The same unscoped public method is also called from unit and integration test crates, and its merge logic duplicates the test-only `update` implementation. The API name and visibility do not distinguish the authorized disk-observation owner from test seeding.

## Proposed Solution

Keep periodic disk refresh as the sole production owner of workspace-observation replacement. Replace the unscoped public `update_with_mode` surface with one private merge helper called by `refresh_from_disk_at`, plus an explicitly named `#[doc(hidden)]` public test-seeding entry point required by integration test crates. Make the existing test-only `update` delegate to the same merge helper. Event-driven updates remain dispatcher-owned.

## Acceptance Criteria

- Production workspace-observation replacement is reachable only through `refresh_from_disk_at`; no external production module calls the retired unscoped update API.
- Unit and integration tests seed equivalent snapshots through one explicitly named, hidden test-support entry point.
- Event-driven projection delivery remains dispatcher-owned, while periodic disk refresh remains the documented non-event observation owner.

## Explicit Completion Conditions

- The scoped production or test code follows the single ownership boundary described above.
- The focused repository-local verification declared as `focused-tests` passes in one bounded invocation.
- The implementation is archived and integrated without unrelated source or contract changes.

## Change Boundary

`src/web/state.rs`, all current unit-test callers under `src/**`, and integration-test callers in `tests/client_cli_tests.rs` and `tests/client_completion_sink.rs`. No production event behavior, periodic refresh policy, or wire contract change.

## Preserved Contracts

Public wire formats, unrelated lifecycle policy, and behavior outside the stated acceptance criteria remain unchanged.

## Failure Behavior

The refactoring must not replace errors with successful placeholder values or silently fall back to a different operation. Existing fail-closed behavior remains fail-closed.

## Out of Scope

Repository-wide redesign, dependency upgrades, unrelated cleanup, deployment, release, and external publication.
