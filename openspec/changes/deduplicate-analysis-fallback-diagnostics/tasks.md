## Implementation Tasks

- [x] Move recoverable fallback tracing and runtime warning emission behind the same existing `DiagnosticDeduplicationKey::AnalysisFailure` decision in `src/parallel_run_service.rs` (verification: unit - extend `test_recoverable_analysis_fallback_diagnostic_dedupes_by_signature` in `src/parallel_run_service.rs` and run `cargo test recoverable_analysis_fallback_diagnostic_dedupes_by_signature` to prove two equivalent emissions yield exactly one WARN tracing record and one warning runtime event).
- [x] Preserve independent visibility for changed rejection reasons and queued/in-flight signatures across both observability surfaces (verification: unit - in `src/parallel_run_service.rs`, capture repeated equivalent, changed-error, and changed-context emissions, then run `cargo test recoverable_analysis_fallback_diagnostic_dedupes_by_signature` to verify matching bounded tracing/event counts).
- [x] Preserve non-fatal classification and fallback execution semantics (verification: integration - run `cargo test recoverable_analysis_fallback_emits_warning_without_terminal_error` from `src/parallel_run_service.rs` to prove an omitted queued ID yields complete metadata fallback order, retained declared dependencies, no ERROR tracing record or terminal error event, and continued execution).
- [x] Run focused and repository quality gates (verification: integration - `cargo test recoverable_analysis_fallback`, `cargo fmt --check`, and `make lint` all pass).

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate deduplicate-analysis-fallback-diagnostics --archive-gate`
