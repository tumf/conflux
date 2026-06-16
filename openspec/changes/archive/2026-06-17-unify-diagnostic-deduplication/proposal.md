---
change_type: implementation
priority: high
dependencies: []
references:
  - "src/parallel/queue_state.rs:2358-2454 (emit_no_analysis_diagnostic, emit_capacity_zero_dispatch_diagnostic_once)"
  - "src/parallel/mod.rs:197-214 (6 HashSets on ParallelExecutor)"
  - "src/parallel/tests/executor.rs:700-1743 (7 repeated HashSet initializations)"
  - "src/tui/state/event_handlers/processing.rs:101-121 (last_logged_analysis_signature)"
  - "openspec/specs/parallel-execution/spec.md:157 (Dependency-blocked diagnostics are stable and non-spamming)"
---

# Unify diagnostic deduplication mechanisms

**Change Type**: implementation

## Problem / Context

The scheduler emits 9 distinct classes of operator-visible diagnostics, each guarded by its own deduplication mechanism:

- `no_analysis_diagnostics_seen` (4-tuple key)
- `dispatch_capacity_zero_diagnostics_seen` (5-tuple key, added by `fix-dispatch-capacity-zero-log-spam`)
- `analyze_failure_diagnostics_seen`
- `dependency_blocker_diagnostics_seen` + `dependency_blocker_fingerprints`
- `queue_reconciliation_diagnostics_seen`
- `last_logged_analysis_signature` (TUI side)
- `last_merge_deferred_diagnostic`
- `last_resolve_wait_base_dirty`

These mechanisms are implemented as 6 separate `HashSet` fields on `ParallelExecutor` (`mod.rs:197-214`), 3 additional fields in `tui/state.rs`, and 1 `Arc<Mutex<HashSet>>` in `parallel_run_service.rs`. Each has its own boilerplate initialization in `builder.rs` and in 7 repeated test setup blocks in `tests/executor.rs`.

Three nearly-identical `emit_*_diagnostic_once` functions (`emit_no_analysis_diagnostic`, `emit_capacity_zero_dispatch_diagnostic_once`, `emit_analyze_failure_diagnostic_once`) duplicate the same "build key → try insert → debug suppress or send event" pattern.

This proliferation is a regression hotspot: every new diagnostic type requires adding a HashSet, wiring it through the builder, updating 7 test helpers, and writing another 40-line `emit_*` function. The recent `fix-dispatch-capacity-zero-log-spam` change added the 6th HashSet.

## Proposed Solution

Introduce a single generic `DiagnosticDeduplicationStore<K>` that encapsulates the seen-set + emit-or-suppress logic. Replace the 6 HashSets on `ParallelExecutor` with a single store instance (or a small number of typed stores). The three `emit_*` functions become thin wrappers around `store.emit_or_suppress(key, event, tx, log_msg)`.

The store remains in-memory and non-authoritative, consistent with Constitution §1. No change to scheduling eligibility, resume routing, or archive decisions.

Add a `MODIFIED Requirements` delta extending the existing "Dependency-blocked diagnostics are stable and non-spamming" requirement to mandate that all operator-visible diagnostics use the unified store.

## Acceptance Criteria

- All 9 diagnostic classes route through a single `DiagnosticDeduplicationStore` implementation.
- The 6 HashSet fields are removed from `ParallelExecutor`; builder and test initialization sites collapse to a single call.
- The three `emit_*_diagnostic_once` functions are replaced by thin wrappers (≤10 LOC each).
- Existing tests (`scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve`, `distinct_same_count_analysis_attempts_both_log_after_merge_wait_queueing`, etc.) continue to pass with identical assertions.
- `cargo test parallel::tests::executor parallel::tests::manual_resolve` passes.
- Strict validation passes.

## Explicit Completion Conditions

- `src/parallel/dedup.rs` (or equivalent) defines `DiagnosticDeduplicationStore<K>` with `emit_or_suppress` and `reset` methods.
- `src/parallel/mod.rs` removes the 6 HashSet fields; `ParallelExecutor` holds at most one `DiagnosticDeduplicationStore` (or a small set of typed stores).
- `src/parallel/builder.rs` and the 7 test setup blocks in `tests/executor.rs` each contain a single initialization site.
- `src/parallel/queue_state.rs` lines 2358-2454 use the unified store for the three diagnostic paths.
- `openspec/changes/unify-diagnostic-deduplication/specs/parallel-execution/spec.md` adds a `MODIFIED Requirements` entry referencing the exact canonical heading at `openspec/specs/parallel-execution/spec.md:157`.
- `cflx openspec validate unify-diagnostic-deduplication --strict --evidence warn` passes.

## Out of Scope

- Changing TUI-side dedup (`last_logged_analysis_signature` etc.) unless it can be removed entirely by the unified store.
- Altering diagnostic message text or severity.
- Persisting dedup state (must remain in-memory per Constitution).
