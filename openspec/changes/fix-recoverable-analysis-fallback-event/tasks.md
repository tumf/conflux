## Implementation Tasks

- [ ] Refactor the recoverable analysis fallback diagnostic in `src/parallel_run_service.rs` so successful metadata fallback emits one deduplicated `ParallelEvent::Log` warning that names the fallback mode and original error, without emitting `ParallelEvent::Error`. Completion: event emission is warning-only while the existing `DiagnosticDeduplicationKey::AnalysisFailure` identity remains stable. (verification: unit - targeted event-channel assertions in `src/parallel_run_service.rs`)
- [ ] Add regression coverage that runs malformed or incomplete LLM analysis through `analyze_order_with_sender` with an event sender and asserts metadata fallback returns all queued changes, preserves declared dependencies, emits the degraded-path warning, and emits no terminal error event. Completion: the test fails for the current warning-plus-error behavior and passes only when real fallback output and event classification are correct. (verification: integration - `cargo test recoverable_analysis_fallback`)
- [ ] Cover diagnostic deduplication and distinct-context behavior for the warning-only fallback event. Completion: equivalent queued/in-flight/error tuples produce one warning, while a changed error or queue context remains visible, with no terminal error event in either case. (verification: unit - targeted dedup tests in `src/parallel_run_service.rs` or the existing parallel executor test module)
- [ ] Preserve terminal and fail-closed behavior outside successful fallback. Completion: existing tests continue to show missing/rejected dependencies block dispatch and genuine scheduler/runtime failures still emit `ParallelEvent::Error`; no production branch converts those failures to warnings. (verification: integration - targeted parallel dependency and executor tests plus `cargo test --all-features`)
- [ ] Run repository quality gates after the focused regression passes. Completion: formatting, lint, and the default non-heavy test suite finish successfully without modifying unrelated behavior. (verification: integration - `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`)

## Future Work

- Retry or deterministic repair of incomplete LLM analysis output requires a separate proposal because safe metadata fallback already preserves execution correctness.

## Final Validation

Expected archive gate: `cflx openspec validate fix-recoverable-analysis-fallback-event --archive-gate`
