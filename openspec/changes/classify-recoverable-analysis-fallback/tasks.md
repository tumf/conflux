## Implementation Tasks

- [ ] Adjust recoverable LLM-analysis fallback logging so the LLM failure plus successful metadata fallback is emitted below error severity while retaining a visible degraded-path diagnostic. (verification: unit - add/update `src/parallel_run_service.rs` tests around `analyze_order_with_sender` or its logging seam; completion condition: a recoverable fallback path has no ERROR-level record but still logs fallback visibility)
- [ ] Preserve metadata-dependency-only fallback behavior. (verification: unit - run or extend the existing fallback dependency preservation test in `src/parallel_run_service.rs`; completion condition: fallback output still contains declared metadata dependencies and does not become dependency-free)
- [ ] Preserve actionable dependency blocker severity for unsafe cases. (verification: integration - run or extend dependency target tests in `src/analyzer.rs` and `src/parallel/tests/executor.rs` covering missing/rejected dependency blockers; completion condition: missing/rejected dependency references still fail closed and remain visible)
- [ ] Confirm observability-only behavior. (verification: manual - inspect `src/parallel_run_service.rs`, `src/parallel/queue_state.rs`, and `scripts/cflx-log-mine.py` to ensure log severity is not read by scheduler dispatch, resume routing, acceptance, archive, merge, or next-action decisions; completion condition: workflow decisions still derive from workspace/git/base state and in-memory scheduler evaluation, not mined logs)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate classify-recoverable-analysis-fallback --archive-gate`
