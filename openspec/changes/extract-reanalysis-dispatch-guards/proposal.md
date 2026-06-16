---
change_type: implementation
priority: high
dependencies:
  - unify-diagnostic-deduplication
references:
  - "src/parallel/queue_state.rs:2472-2770 (perform_reanalysis_and_dispatch)"
  - "src/parallel/orchestration.rs:86-319 (scheduler loop)"
  - "src/parallel/dynamic_queue.rs (ReanalysisReason and debounce)"
  - "openspec/specs/parallel-execution/spec.md:1197 (Re-analysis triggers and non-blocking scheduler)"
  - "openspec/specs/parallel-execution/spec.md:1286 (In-flight tracking and slot-based dispatch)"
---

# Extract reanalysis dispatch guards from scheduler core

**Change Type**: implementation

## Problem / Context

`ParallelExecutor::perform_reanalysis_and_dispatch` is a 299-line function with at least 19 mixed responsibilities:

1. cancellation checks
2. queued-work classification
3. blocked-only early returns
4. no-dispatchable-candidate early returns
5. queue trimming
6. slot tracking
7. slot recovery detection
8. `ReanalysisReason` transformation
9. debounce checks
10. failed-dependency filtering
11. skip event emission
12. empty-queue early returns
13. dependency analysis execution
14. empty analysis result handling
15. dependency-tracker mutation
16. post-analysis slot recalculation
17. capacity-zero dispatch suppression
18. dispatch selection
19. workspace dispatch loop + queue cleanup

The function is already marked with `#[allow(clippy::too_many_arguments)]`, indicating known complexity. Recent regressions repeatedly occurred in this function because small behavioral fixes had to be inserted into the same long control-flow chain.

## Proposed Solution

Extract guard/decision subroutines from `perform_reanalysis_and_dispatch` while preserving behavior:

- `prepare_dispatch_candidates()`
  - classification
  - blocked-only/no-dispatchable early decisions
  - queue trimming
- `compute_effective_reanalysis_reason()`
  - slot recovery detection
  - queue notification promotion
- `should_run_analysis_now()`
  - debounce checks
  - first-iteration behavior
- `filter_executable_candidates()`
  - failed dependency filtering
  - skip event emission
- `run_dependency_analysis_attempt()`
  - `AnalysisStarted` event
  - analyzer invocation
  - empty order handling
- `handle_post_analysis_capacity()`
  - post-analysis slot recalculation
  - capacity-zero diagnostic and dispatch suppression
- `dispatch_selected_candidates()`
  - select changes
  - dispatch loop
  - local queue cleanup

The public behavior must remain identical. This is a refactoring proposal, not a scheduling policy change.

## Acceptance Criteria

- `perform_reanalysis_and_dispatch` is reduced from ~299 lines to ≤80 lines and reads as an orchestration skeleton.
- Each extracted helper has a single explicit responsibility and returns typed decisions instead of mixing early returns with side effects where practical.
- Existing zero-capacity behavior remains: analysis runs, dispatch is suppressed, diagnostic emitted once.
- Existing queue notification behavior remains: `QueueNotification` bypasses debounce after initial iteration.
- Existing blocked-only behavior remains: analyzer is not invoked for merge-wait/terminal-error-only candidates.
- Existing tests for manual resolve, auto resolve, dynamic queue, and executor behavior pass.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` contains extracted helper functions or a new `src/parallel/reanalysis.rs` module with the listed responsibilities.
- `perform_reanalysis_and_dispatch` has ≤80 lines excluding comments and delegates to helpers.
- `#[allow(clippy::too_many_arguments)]` is removed or the argument list is wrapped in a `ReanalysisDispatchContext` struct.
- `cargo test parallel::tests::executor::test_queue_notification_with_fresh_debounce_starts_analysis_after_initial_iteration` passes.
- `cargo test parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` passes.
- `cargo test parallel::tests::executor::test_blocked_only_reanalysis_skips_analyzer_for_merge_wait_and_terminal_error` passes.
- `cflx openspec validate extract-reanalysis-dispatch-guards --strict --evidence warn` passes.

## Dependencies

Depends on `unify-diagnostic-deduplication` because the capacity-zero and no-analysis diagnostic calls should be stabilized before reshaping `perform_reanalysis_and_dispatch`.

## Out of Scope

- Changing scheduler policy, debounce durations, or dispatch ordering.
- Changing merge/retry lane semantics.
- Splitting `orchestration.rs` scheduler loop; this proposal only extracts the inner reanalysis/dispatch function.
