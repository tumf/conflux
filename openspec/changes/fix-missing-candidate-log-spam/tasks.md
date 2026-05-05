## Implementation Tasks

- [x] Task 1: Bound structured missing-candidate diagnostics in `src/parallel/queue_state.rs`. (verification: unit - added `test_scheduler_reconciliation_missing_candidate_warn_is_observable_but_bounded` in `src/parallel/tests/executor.rs`; focused run `cargo test test_scheduler_reconciliation_missing_candidate_warn_is_observable_but_bounded` passed and asserts one TUI-visible warn plus one structured WARN across repeated reconciliation)
- [x] Task 2: Preserve queue reconciliation behavior for missing and loadable candidates. (verification: integration - `cargo test scheduler_reconciliation` passed; `test_scheduler_reconciliation_missing_candidate_warn_is_observable_but_bounded` asserts missing candidates are not pushed into scheduler-local `queued` and the loadable reducer-queued change `fix-missing-candidate-log-spam` is added exactly once)
- [x] Task 3: Confirm observability-only suppression state. (verification: manual - inspected `src/parallel/queue_state.rs`: suppression uses existing in-memory `queue_reconciliation_diagnostics_seen` via `should_emit_queue_reconciliation_diagnostic` only around diagnostic emission; the scheduling branches still skip missing candidates and push loadable candidates independently of suppression, with no durable state writes or resume/archive/merge routing changes. The focused test in `src/parallel/tests/executor.rs` confirms semantics.)
- [x] Task 4: Run repository formatting and targeted Rust verification for the touched modules. (verification: manual - `cargo fmt --check` passed; `cargo test scheduler_reconciliation` passed; `cflx openspec validate fix-missing-candidate-log-spam --strict --evidence warn` passed)

## Future Work

- Broader log-mining UX improvements are intentionally separate from this fix.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate fix-missing-candidate-log-spam --archive-gate`
