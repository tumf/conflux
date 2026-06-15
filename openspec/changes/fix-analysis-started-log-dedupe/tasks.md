## Implementation Tasks

- [ ] Update the analysis-started observability model so distinct analysis attempts are not deduped solely by `remaining_changes`; completion condition: `src/tui/state/event_handlers/processing.rs` or the event payload it consumes uses an identity that distinguishes separate analysis attempts with the same remaining count. (verification: unit - add or update tests in `src/tui/state/event_handlers/processing.rs` proving two distinct same-count analysis attempts both log)

- [ ] Preserve duplicate suppression for repeated delivery of the same analysis attempt; completion condition: repeated identical analysis-started events for one attempt still produce only one TUI log entry. (verification: unit - update `src/tui/state/event_handlers/processing.rs` repeated-analysis tests to use the new dedupe identity)

- [ ] Keep queue debounce and scheduler behavior unchanged for `x` queued operations; completion condition: no implementation treats `TuiCommand::AddToQueue` or `DynamicQueue::push` as an immediate analyze bypass, and debounce remains enforced by `should_reanalyze_queue` except existing bypass cases. (verification: unit - run or extend debounce tests in `src/parallel/tests/executor.rs` and `src/tui/queue.rs`)

- [ ] Add a regression test for `MergeWait` followed by a separately queued single change with the same remaining count; completion condition: the test models an earlier `AnalysisStarted` for one change, a `MergeDeferred`/`MergeWait` transition, then a later distinct `AnalysisStarted` for one change, and observes two user-visible analysis logs. (verification: unit - add a test in `src/tui/state/event_handlers/processing.rs`, `src/tui/state/event_handlers/errors.rs`, or a nearby TUI state test module)

- [ ] Verify scheduler event emission still reaches TUI through the existing forwarding path; completion condition: `ParallelEvent::AnalysisStarted` / `ExecutionEvent::AnalysisStarted` propagation remains wired through `src/tui/orchestrator.rs` and `src/tui/state/event_handlers/mod.rs`. (verification: integration - run targeted TUI/parallel tests such as `cargo test scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve`, or manually confirm `~/.local/state/cflx/logs/**/YYYY-MM-DD.log` contains `Re-analysis triggered` and the TUI shows a corresponding analysis log)

- [ ] Run formatting, linting, and targeted tests for affected modules; completion condition: Rust formatting is clean and affected unit tests pass. (verification: unit - `cargo fmt --check` and targeted `cargo test` commands covering TUI event handlers and parallel scheduler tests)

## Future Work

- Manual observation in a live TUI run can confirm the operator-visible log appears after queue debounce, but the implementation must not rely solely on manual validation.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-analysis-started-log-dedupe --archive-gate`
