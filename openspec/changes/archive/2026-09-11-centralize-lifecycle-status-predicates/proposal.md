---
change_type: implementation
priority: high
dependencies: []
references:
  - "src/web/remote_control_api/tests/operator_snapshot_tests.rs"
verifications:
  - id: focused-tests
    requirement: The refactoring preserves the named behavior and proves the corrected boundary
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/operator_snapshot_tests.rs
    evidence: cargo test --lib web::remote_control_api::tests::operator_snapshot_tests && cargo test --lib web::remote_control_api::tests::projection_tests && cargo test --lib tui::render
    rerun: cargo test --lib web::remote_control_api::tests::operator_snapshot_tests && cargo test --lib web::remote_control_api::tests::projection_tests && cargo test --lib tui::render
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Centralize lifecycle status predicates

**Change Type**: implementation

## Problem / Context

Web snapshot totals and TUI action hints spell lifecycle status sets independently. `pushed` and `rejecting` are handled inconsistently, so equivalent projections can report different totals or actions.

## Proposed Solution

Define two predicates beside the existing lifecycle constants in `src/orchestration/operator_command.rs`: completed is exactly the existing post-archive set `{archived, merged, pushed}`, and in-progress is exactly `ACTIVE_STATUSES`. First correct `refresh_summary` to use them, then make initial Web summary delegate to that corrected implementation. Reuse the same predicates only for TUI post-archive checks, `push_change_row_hints`, and header active counts; per-status spinner and badge formatting remains unchanged.

## Acceptance Criteria

- `pushed` contributes to completed totals in every Web snapshot construction path.
- `rejecting` contributes to in-progress totals and receives the same active-change hint policy as the shared classifier.
- No wire schema or lifecycle transition changes.

## Explicit Completion Conditions

- The scoped production or test code follows the single ownership boundary described above.
- The focused repository-local verification declared as `focused-tests` passes in one bounded invocation.
- The implementation is archived and integrated without unrelated source or contract changes.

## Change Boundary

`src/orchestration/operator_command.rs`, `src/web/state.rs`, `src/tui/render.rs`, and focused tests only.

## Preserved Contracts

Public wire formats, unrelated lifecycle policy, and behavior outside the stated acceptance criteria remain unchanged.

## Failure Behavior

The refactoring must not replace errors with successful placeholder values or silently fall back to a different operation. Existing fail-closed behavior remains fail-closed.

## Out of Scope

Repository-wide redesign, dependency upgrades, unrelated cleanup, deployment, release, and external publication.
