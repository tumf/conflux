---
change_type: implementation
priority: medium
dependencies: []
references:
  - "src/parallel/queue_state.rs:1475-1618 (classify_queued_work)"
  - "src/parallel/queue_state.rs:1767-1945 (select_changes_for_dispatch)"
  - "src/parallel/queue_state.rs:639-661 (select_changes_for_dispatch dependency blocker loop)"
  - "src/parallel/queue_state.rs:474-491 (is_dependency_resolved_with_base)"
---

# Unify dependency classification logic

**Change Type**: implementation

## Problem / Context

`classify_queued_work` (141 lines) and `select_changes_for_dispatch` (176 lines) both build identical dependency context sets:

- `queued_ids: HashSet<&str>`
- `in_flight_ids: HashSet<&str>`
- `active_ids` / `active_refs`
- `archived_ids`
- `rejected_ids`
- terminal error checks via `shared_orchestrator_state`

Both then loop over changes calling `classify_dependency_target` with the same dispatch table, read the same shared state for `is_terminal_error_change`, and produce overlapping results (`DependencyTargetClassification`, blocker fingerprints, etc.).

This duplication is a regression vector: a change to dependency classification semantics (e.g., the recent `fix-archived-dependency-effective-base` change) must be applied in two places. Missing one causes inconsistent dispatch vs. analysis behavior.

## Proposed Solution

Introduce a `DependencyContext<'a>` struct (or module) that encapsulates the six HashSets and provides methods:

- `from(queued, in_flight, shared_state) -> DependencyContext`
- `classify(change_id) -> DependencyTargetClassification`
- `is_blocked(change_id) -> Option<DependencyBlockerFingerprint>`
- `effective_base() -> Option<String>` (from `fix-archived-dependency-effective-base`)

`classify_queued_work` and `select_changes_for_dispatch` become thin callers of the shared context. Existing behavior must remain unchanged.

## Acceptance Criteria

- `classify_queued_work` and `select_changes_for_dispatch` no longer duplicate the six HashSet constructions or the `classify_dependency_target` loop.
- A single `DependencyContext` implementation is used by both functions.
- `effective_dependency_base` logic (current-branch vs. original-branch) is encapsulated inside the context.
- Existing tests for archived-dependency blocking, terminal-error handling, and manual resolve continue to pass with identical assertions.
- No behavioral change to dispatch eligibility or blocker diagnostics.

## Explicit Completion Conditions

- `src/parallel/dependency.rs` (or equivalent) defines `DependencyContext` with the listed methods.
- `src/parallel/queue_state.rs` lines 1475-1618 and 1767-1945 delegate classification to `DependencyContext`.
- `cargo test parallel::tests::executor::test_single_queued_archived_dependency_waits_until_merged` passes.
- `cargo test parallel::tests::executor::test_archived_dependency_uses_effective_integration_base_after_startup` passes.
- `cargo test parallel::tests::manual_resolve::test_manual_resolve_zero_capacity_runs_analysis_but_suppresses_apply_dispatch` passes.
- `cflx openspec validate unify-dependency-classification --strict --evidence warn` passes.

## Dependencies

Depends on `unify-diagnostic-deduplication` and `extract-reanalysis-dispatch-guards` to ensure the diagnostic store and reanalysis skeleton are stable before refactoring the classification core.

## Out of Scope

- Changing dependency classification semantics or blocker fingerprint format.
- Altering `is_merged_to_base` or `effective_dependency_base` policy.
- Modifying the TUI or reducer representation of blocker state.
