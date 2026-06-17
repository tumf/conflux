## Implementation Tasks

- [x] Extract `prepare_dispatch_candidates(&mut self, queued, in_flight)` from `perform_reanalysis_and_dispatch` that performs classification, blocked-only/no-dispatchable checks, and returns trimmed dispatchable queued list + early-exit decision. (verification: unit - `cargo test parallel::tests::executor::test_blocked_only_reanalysis_skips_analyzer_for_merge_wait_and_terminal_error` still asserts analyzer is skipped)
- [x] Extract `compute_effective_reanalysis_reason(...)` that encapsulates slot-recovery detection and `ReanalysisReason` promotion logic. (verification: unit - `cargo test parallel::tests::executor::test_should_reanalyze_bypasses_debounce_on_slot_recovery` continues to assert `SlotRecovery` promotion)
- [x] Extract `should_run_analysis_now(...)` that encapsulates debounce + first-iteration bypass. (verification: unit - `cargo test parallel::tests::executor::test_queue_notification_with_fresh_debounce_starts_analysis_after_initial_iteration` still asserts analysis starts for `QueueNotification` at iteration 2)
- [x] Extract `filter_executable_candidates(...)` that performs failed-dependency filtering and skip event emission. (verification: integration - `cargo test parallel::tests::executor::test_dependency_on_terminal_error_is_blocked_until_retry_and_success` passes)
- [x] Extract `run_dependency_analysis_attempt(...)` that wraps `AnalysisStarted`, analyzer invocation, and empty-order handling. (verification: integration - `cargo test parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` still observes `AnalysisStarted`)
- [x] Extract `handle_post_analysis_capacity(...)` that performs post-analysis slot recalculation and capacity-zero suppression. (verification: integration - zero-capacity dispatch suppression tests continue to pass)
- [x] Refactor `perform_reanalysis_and_dispatch` to call the extracted helpers, reducing body to ≤80 non-comment lines. (verification: manual - `cargo clippy -- -D warnings` reports no `too_many_arguments` allow on the function)
- [x] Add spec delta under `openspec/changes/extract-reanalysis-dispatch-guards/specs/parallel-execution/spec.md`. (verification: unit - `cflx openspec validate extract-reanalysis-dispatch-guards --strict` passes)

## Future Work

- Consider extracting the scheduler loop in `orchestration.rs` into a state-machine or explicit `SchedulerLoop` struct after the inner dispatch guards are stabilized.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate extract-reanalysis-dispatch-guards --archive-gate`
