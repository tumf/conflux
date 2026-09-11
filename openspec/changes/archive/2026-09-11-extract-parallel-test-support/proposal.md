---
change_type: implementation
priority: medium
dependencies: []
references:
  - "src/parallel/tests/mod.rs"
verifications:
  - id: focused-tests
    requirement: The refactoring preserves the named behavior and proves the corrected boundary
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/mod.rs
    evidence: cargo test --lib parallel::tests && cargo test --lib parallel_run_service
    rerun: cargo test --lib parallel::tests && cargo test --lib parallel_run_service
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Extract shared parallel test support

**Change Type**: implementation

## Problem / Context

`src/parallel/tests/executor.rs` is a 16k-line hotspot and owns fixtures imported by sibling modules. Git repository setup and test configuration helpers are duplicated, preventing incremental thematic splits.

## Proposed Solution

Create `src/parallel/tests/support.rs` and move shared test-only fixtures and helpers there. Repoint sibling imports and remove duplicate helpers without changing production code, test assertions, or test inventory.

## Acceptance Criteria

- Shared workspace, config, Git setup, and failure helpers live in `support.rs`.
- Sibling parallel tests no longer import fixtures from `executor.rs`.
- Parallel test count and assertions remain unchanged.

## Explicit Completion Conditions

- The scoped production or test code follows the single ownership boundary described above.
- The focused repository-local verification declared as `focused-tests` passes in one bounded invocation.
- The implementation is archived and integrated without unrelated source or contract changes.

## Change Boundary

`src/parallel/tests/**` only. `src/parallel_run_service.rs` is unchanged and its tests are run only as a regression guard. Production code is out of scope.

## Preserved Contracts

Public wire formats, unrelated lifecycle policy, and behavior outside the stated acceptance criteria remain unchanged.

## Failure Behavior

The refactoring must not replace errors with successful placeholder values or silently fall back to a different operation. Existing fail-closed behavior remains fail-closed.

## Out of Scope

Repository-wide redesign, dependency upgrades, unrelated cleanup, deployment, release, and external publication.
