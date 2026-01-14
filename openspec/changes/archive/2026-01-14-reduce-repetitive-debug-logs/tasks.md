# Tasks: Reduce Repetitive Debug Logs

## Implementation Tasks

- [x] Create log deduplicator module
  - Create `src/tui/log_deduplicator.rs`
  - Implement `ChangeStateSnapshot` struct
  - Implement `LogDeduplicator` with HashMap-based state tracking
  - Add `should_log()` method with state comparison

- [x] Add configuration support
  - Update `src/config/defaults.rs` with logging section defaults
  - Add `LoggingConfig` struct in `src/config/mod.rs`
  - Parse `suppress_repetitive_debug` and `summary_interval_secs` options

- [x] Integrate deduplicator in task parser
  - Modify `src/task_parser.rs:79` to check before logging
  - Pass change_id and task progress to deduplicator
  - Ensure state changes are still logged

- [x] Integrate deduplicator in approval module
  - Modify `src/approval.rs:159` (not approved case)
  - Modify `src/approval.rs:245` (approved case)
  - Track approval status changes per change_id

- [x] Integrate deduplicator in openspec module
  - Modify `src/openspec.rs:199` to deduplicate change count logs
  - Consider tracking individual change list changes

- [x] Add periodic summary logging
  - Implement `maybe_log_summary()` in deduplicator
  - Call from TUI update loop or orchestrator
  - Use configured summary interval

- [x] Add unit tests
  - Test state change detection (same state → no log)
  - Test state transition detection (different state → log)
  - Test multiple changes tracked independently
  - Test summary interval logic

- [x] Add integration tests
  - Mock TUI mode execution with repetitive state
  - Verify log suppression works
  - Measure log volume reduction

- [x] Documentation
  - Update README with logging configuration
  - Add comments explaining deduplication logic
  - Document configuration options in config schema

## Acceptance Criteria

- ✓ Repetitive DEBUG logs are suppressed when state unchanged
- ✓ State transitions are still logged immediately
- ✓ Periodic summary logs appear every N seconds (configurable)
- ✓ Configuration option `suppress_repetitive_debug` works
- ✓ Unit tests pass with >80% coverage
- ✓ Log file size reduced by >90% in typical scenarios
- ✓ No performance degradation in TUI mode
