## Implementation Tasks

- [x] Update the analysis-started observability model so distinct analysis attempts are not deduped solely by `remaining_changes`; completion condition: `src/tui/state/event_handlers/processing.rs` or the event payload it consumes uses an identity that distinguishes separate analysis attempts with the same remaining count. (verification: unit - added `attempt_id` to `ExecutionEvent::AnalysisStarted`, generated it from iteration/trigger/queued IDs in `src/parallel/queue_state.rs`, and verified with `cargo test tui::state::event_handlers::processing::tests --lib`)

- [x] Preserve duplicate suppression for repeated delivery of the same analysis attempt; completion condition: repeated identical analysis-started events for one attempt still produce only one TUI log entry. (verification: unit - updated repeated-analysis tests to use identical `(remaining_changes, attempt_id)` signatures and verified with `cargo test tui::state::event_handlers::processing::tests --lib`)

- [x] Keep queue debounce and scheduler behavior unchanged for `x` queued operations; completion condition: no implementation treats `TuiCommand::AddToQueue` or `DynamicQueue::push` as an immediate analyze bypass, and debounce remains enforced by `should_reanalyze_queue` except existing bypass cases. (verification: unit - no `AddToQueue`/`DynamicQueue::push` analyze bypass was added; verified debounce behavior with `cargo test test_should_reanalyze_bypasses_debounce_on_slot_recovery --lib` and `cargo test test_debounce_with_queue_changes --lib`)

- [x] Add a regression test for `MergeWait` followed by a separately queued single change with the same remaining count; completion condition: the test models an earlier `AnalysisStarted` for one change, a `MergeDeferred`/`MergeWait` transition, then a later distinct `AnalysisStarted` for one change, and observes two user-visible analysis logs. (verification: unit - added `distinct_same_count_analysis_attempts_both_log_after_merge_wait_queueing` in `src/tui/state/event_handlers/processing.rs` and verified with `cargo test tui::state::event_handlers::processing::tests --lib`)

- [x] Verify scheduler event emission still reaches TUI through the existing forwarding path; completion condition: `ParallelEvent::AnalysisStarted` / `ExecutionEvent::AnalysisStarted` propagation remains wired through `src/tui/orchestrator.rs` and `src/tui/state/event_handlers/mod.rs`. (verification: integration - updated `src/tui/state/event_handlers/mod.rs` to forward `attempt_id` and verified scheduler/TUI propagation with `cargo test scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve --lib`)

- [x] Run formatting, linting, and targeted tests for affected modules; completion condition: Rust formatting is clean and affected unit tests pass. (verification: unit - `cargo fmt --check`; `cargo test tui::state::event_handlers::processing::tests --lib`; `cargo test test_should_reanalyze_bypasses_debounce_on_slot_recovery --lib`; `cargo test test_debounce_with_queue_changes --lib`; `cargo test scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve --lib`)

## Future Work

- Manual observation in a live TUI run can confirm the operator-visible log appears after queue debounce, but the implementation must not rely solely on manual validation.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-analysis-started-log-dedupe --archive-gate`
