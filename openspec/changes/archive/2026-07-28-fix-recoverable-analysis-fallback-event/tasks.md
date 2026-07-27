## Implementation Tasks

- [x] Refactor the recoverable analysis fallback diagnostic in `src/parallel_run_service.rs` so successful metadata fallback emits one deduplicated `ParallelEvent::Log` warning that names the fallback mode and original error, without emitting `ParallelEvent::Error`. Completion: event emission is warning-only while the existing `DiagnosticDeduplicationKey::AnalysisFailure` identity remains stable. (verification: unit - targeted event-channel assertions in `src/parallel_run_service.rs`)
- [x] Add regression coverage that runs malformed or incomplete LLM analysis through `analyze_order_with_sender` with an event sender and asserts metadata fallback returns all queued changes, preserves declared dependencies, emits the degraded-path warning, and emits no terminal error event. Completion: the test fails for the current warning-plus-error behavior and passes only when real fallback output and event classification are correct. (verification: integration - `cargo test recoverable_analysis_fallback`)
- [x] Cover diagnostic deduplication and distinct-context behavior for the warning-only fallback event. Completion: equivalent queued/in-flight/error tuples produce one warning, while a changed error or queue context remains visible, with no terminal error event in either case. (verification: unit - targeted dedup tests in `src/parallel_run_service.rs` or the existing parallel executor test module)
- [x] Preserve terminal and fail-closed behavior outside successful fallback. Completion: existing tests continue to show missing/rejected dependencies block dispatch and genuine scheduler/runtime failures still emit `ParallelEvent::Error`; no production branch converts those failures to warnings. (verification: integration - targeted parallel dependency and executor tests plus `cargo test --all-features`)
- [x] Run repository quality gates after the focused regression passes. Completion: formatting, lint, and the default non-heavy test suite finish successfully without modifying unrelated behavior. (verification: integration - `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`)

## Notes

- Verification evidence:
  - `cargo test --lib recoverable_analysis_fallback` — 3 passed. Reverting only the warning-only emission (re-adding the `ParallelEvent::Error` send) makes 2 of the 3 fail, confirming the regression tests are behavior-bound rather than tautological.
  - Targeted dependency/blocker/fail-closed suites (`cargo test --lib -- dependency analysis_failure blocker fail_closed`) — 88 passed; `test_analyze_failure_diagnostic_dedupes_by_signature` still passes, so terminal error diagnostics keep their event semantics.
  - `cargo fmt --check` — clean. `cargo clippy --all-targets --all-features -- -D warnings` — clean.
  - `cargo test` (default non-heavy suite) — all 15 test binaries pass, 0 failures.
- Pre-existing unrelated failure, not caused by this change: under `cargo test --all-features`, `tests/e2e_proposal_session.rs::proposal_session_prompt_injects_backend_managed_spec_guidance` fails (and `proposal_session_reconnect_new_socket_continues_streaming_after_old_disconnect` fails intermittently). Both were reproduced on the unmodified baseline by restoring `git show HEAD:src/parallel_run_service.rs` before running the test, so they are outside this change's scope.

## Future Work

- Retry or deterministic repair of incomplete LLM analysis output requires a separate proposal because safe metadata fallback already preserves execution correctness.

## Final Validation

Expected archive gate: `cflx openspec validate fix-recoverable-analysis-fallback-event --archive-gate`
