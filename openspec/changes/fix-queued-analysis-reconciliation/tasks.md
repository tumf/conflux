## Implementation Tasks

- [x] Task 1: Map the current queued-intent sources and eligibility boundaries for scheduler reconciliation. Completion condition: document in code-adjacent design notes which reducer states, runtime activity states, and OpenSpec change loading outcomes make a change eligible or ineligible for scheduler-local analysis candidates. (verification: not-testable - design evidence in `design.md` plus references to `src/orchestration/state.rs`, `src/parallel/orchestration.rs`, and `src/parallel/queue_state.rs`)

- [x] Task 2: Implement reducer-visible queued candidate reconciliation before scheduler drain/idle and re-analysis decisions. Completion condition: `execute_with_order_based_reanalysis()` or a helper it calls refreshes scheduler-local queued candidates from shared reducer state and loadable OpenSpec changes before checking `work_drained`, `should_exit_when_idle`, and `!queued.is_empty()`. (verification: integration - add/run a scheduler-level test in `src/parallel/tests/executor.rs` that sets reducer queued intent while local `queued` is empty and asserts `AnalysisStarted` or analyzer invocation occurs)

- [x] Task 3: Make dynamic queue ingestion recoverable rather than permanently authoritative. Completion condition: a dynamic queue pop that is skipped because a change is temporarily not found, already active, or otherwise not yet eligible does not permanently prevent later reducer-state reconciliation from analyzing the same queued change when it becomes eligible. (verification: unit/integration - add tests in `src/parallel/tests/executor.rs` or a focused queue-state test covering candidate-not-found and stale-active recovery)

- [x] Task 4: Preserve analysis targeting and slot semantics while adding reconciliation. Completion condition: analysis still receives only queued, non-terminal, non-active changes; available-slot gating still prevents analysis when all slots are consumed; existing direct queued dispatch tests continue to pass. (verification: unit - run existing and new tests in `src/parallel/tests/executor.rs` covering `test_resolving_with_free_slot_still_dispatches_queued_change`, `test_dispatch_zero_reanalysis_is_retried_on_next_loop`, and no-slot deferral behavior via targeted `cargo test parallel::tests::executor`)

- [x] Task 5: Add observable no-analysis diagnostics for reducer-visible queued work. Completion condition: when reducer-visible queued changes exist but analysis does not start, the scheduler emits a structured log/event with an explicit reason such as no available slots, debounce active, candidate not found, already active, or local queue empty after reconciliation. (verification: unit/integration - add an event/log assertion test for at least one diagnostic reason in `src/parallel/tests/executor.rs` or an equivalent test module)

- [x] Task 6: Ensure TUI queue operations notify and record queued intent consistently without becoming the only source of truth. Completion condition: Space/bulk queue paths keep reducer queued intent and scheduler wake-up behavior aligned, while scheduler reconciliation remains able to recover if notification is missed. (verification: integration - add/run TUI queue or command handler tests under `src/tui/` demonstrating queued intent survives until scheduler ingestion)

- [x] Task 7: Run validation and targeted Rust verification. Completion condition: OpenSpec validation and targeted tests pass, and any slow tests are optimized or marked heavy per repository policy. (verification: integration - `cflx openspec validate fix-queued-analysis-reconciliation --strict --evidence warn`; `cargo fmt --check`; targeted `cargo test` commands for new scheduler and TUI tests)

## Future Work

- Manual reproduction in a real long-running TUI session can be used as extra confidence, but the proposal must be accepted based on repository-verifiable tests and logs, not manual observation alone.

## Acceptance #1 Failure Follow-up

- [x] Task 2 / explicit completion condition is now repository-verified at the scheduler-loop boundary: `src/parallel/tests/executor.rs` includes `test_scheduler_loop_reanalysis_with_reducer_queued_intent`, which starts from reducer queued intent with an empty scheduler-local queue, runs `execute_with_order_based_reanalysis()`, and asserts `AnalysisStarted` is observed without relying on dynamic queue notification.
- [x] The real scheduler loop now emits the required `no_available_slots` diagnostic: `src/parallel/orchestration.rs` emits `emit_no_analysis_diagnostic(..., "no_available_slots")` in the scheduler-loop no-slot branch, and `src/parallel/tests/executor.rs` verifies this behavior in `test_scheduler_emits_no_analysis_diagnostic_when_slots_unavailable` at loop level.

## Acceptance #2 Failure Follow-up

- [ ] The `no_available_slots` diagnostic fix is implemented in the real scheduler loop but still lacks repository-verified loop-level coverage: `src/parallel/orchestration.rs:193-224` now emits `emit_no_analysis_diagnostic(..., "no_available_slots")` when `available_slots == 0`, but `src/parallel/tests/executor.rs:3313-3369` still tests only the direct `perform_reanalysis_and_dispatch()` helper path. Add or adjust a test that runs `execute_with_order_based_reanalysis()` with reducer-visible queued intent, an empty/local reconciled queue, and a saturated slot/in-flight state, then asserts the `reason=no_available_slots` diagnostic is observed from the scheduler loop skip branch.
