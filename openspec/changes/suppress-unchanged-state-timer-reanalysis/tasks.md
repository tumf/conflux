## Implementation Tasks

- [x] Define a deterministic process-local analysis-input signature covering sorted queued analysis fields, stable content digests for both queued and in-flight proposal paths, sorted in-flight IDs, capacity inputs, and effective dependency-base revision; completion requires same-ID queued/in-flight proposal edits and base revision changes to produce unequal signatures without adding durable state. (verification: unit - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Extend the internal analyzer result contract with runtime-only healthy, intentionally metadata-only, and recoverable-failure fallback provenance; completion requires the scheduler to distinguish degraded fallback without changing dependency result semantics or persisting provenance. (verification: unit - `cargo test parallel_run_service::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Wire signature capture after queue reconciliation/classification and immediately before analyzer invocation, then store the captured pre-analysis signature only under the specified healthy/degraded completion conditions; completion requires queue changes during analysis to remain visible as a different next-probe signature. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Gate only ordinary non-bypass `Initial` timer analysis when the current signature equals the last completed signature, while preserving fresh queue classification/reconciliation and emitting a deduplicated `unchanged_analysis_input` reason; completion requires no analyzer call or apply dispatch for a suppressed pass and no proposal/VCS fingerprint probe before the ten-second suppressed-state probe deadline. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Preserve immediate one-shot behavior for queue additions, completion, repair-candidate, and slot-recovery reasons so each real edge bypasses a matching signature once and subsequent timer-only wakes return to unchanged-input suppression. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Preserve autonomous liveness by invalidating the signature when capacity, in-flight membership, queued analysis input, or effective dependency-base evidence changes; completion requires capacity recovery to reach analysis and dispatch evaluation even in a deterministic test variant that does not rely on a retained `SlotRecovery` reason. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Add a paused-time loop regression reproducing the v0.6.200 live state with stale queue debounce, full capacity, stable queued work, and repeated 500 ms wakes; completion requires exactly one analyzer invocation across many wakes for both sub-ten-second and over-ten-second mocked analysis durations. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Add regressions for same-ID queued and in-flight proposal input changes, effective-base revision changes, analysis-time queue changes, and fresh executor startup so each legitimate input transition re-arms analysis while unchanged healthy state remains quiescent. (verification: unit - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Add fallback coverage proving recoverable-failure metadata fallback suppresses rapid unchanged retries, expires after exactly five paused-time minutes, permits one retry, and becomes non-expiring after a healthy result; intentionally metadata-only mode remains non-degraded and an unusable no-result path establishes no signature. (verification: integration - `cargo test parallel::tests --lib parallel_run_service::tests`; verification-id: scheduler-analysis-gate-tests)
- [x] Add liveness coverage where queued work has positive capacity, no in-flight work, and analysis selects zero dispatches; completion requires no suppression record and another analyzer invocation at the next debounced timer evaluation. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Add signature failure and cost coverage by injecting proposal-read and revision-resolution errors and counting revision probes; completion requires fail-open analysis without panic/recording and no VCS probe on intervening 500 ms wakes before the ten-second probe deadline. (verification: integration - `cargo test parallel::tests --lib`; verification-id: scheduler-analysis-gate-tests)
- [x] Run formatting, linting, and default-path tests; completion requires `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` to pass, with timer tests using paused Tokio time and every default test remaining within repository duration policy. (verification: integration - `cargo fmt --check && cargo clippy -- -D warnings && cargo test`; verification-id: scheduler-analysis-gate-tests)

## Notes

Verification evidence for the gate above:
`cargo fmt --check` is clean.
`cargo clippy --all-targets -- -D warnings` is clean.
`cargo test --lib` reports 2490 passed, 0 failed, 7 ignored, including the 19 new
`parallel::tests::unchanged_analysis_input` regressions and the extended
`parallel_run_service::tests` provenance assertions.
Integration targets `completion_command_tests`, `e2e_git_worktree_tests`, `e2e_proposal_session`,
`e2e_tests`, `install_skills_test`, `logs_command_tests`, `merge_conflict_check_tests`,
`no_backup_files_test`, `process_cleanup_test`, and `run_exit_tests` all pass.
Every new test uses paused Tokio time and finishes well under one second (slowest: 0.52s).
Suppression timing uses `tokio::time::Instant`, so paused-time advances drive the ten-second probe
interval and the five-minute degraded interval deterministically.

Pre-existing unrelated failure: `cargo test --test lifecycle_integration` fails on
`adapter_payloads_stay_within_the_privacy_boundary`,
`recording_adapter_receives_ordered_versioned_lifecycle_stream`, and
`adapter_inherits_the_cflx_process_environment`. This was confirmed pre-existing by stashing this
change and re-running the same target on the untouched tree, where the same tests still fail. That
target exercises the lifecycle adapter subprocess and touches no scheduler, analyzer, or signature
code path.

## Future Work

- Add a long-period safety retry only if later evidence identifies dependency-analysis input that cannot be represented by repository-visible signature components; do not add a speculative periodic retry in this change.
- Investigate and fix the pre-existing `tests/lifecycle_integration.rs` adapter-stream failures; they are outside this change's scope and were failing before it.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate suppress-unchanged-state-timer-reanalysis --archive-gate`

## Current Acceptance Follow-up
- attempt: 1
- [ ] src/parallel/tests/executor.rs lines 1152, 1312, 1527, 1734, 1947: heavy-tests-gated ParallelExecutor struct literals are missing new fields last_completed_analysis_input, next_analysis_signature_probe_at, analysis_input_probe; cargo clippy --locked --all-targets --all-features -- -D warnings fails with E0063 and this command runs in the prek pre-commit hook (.pre-commit-config.yaml clippy hook), blocking the archive commit. Add the three fields initialized to None to each initializer, then verify with the exact hook command.
  evidence: Added `last_completed_analysis_input: None`, `next_analysis_signature_probe_at: None`, `analysis_input_probe: None` to all five heavy-tests-gated `ParallelExecutor` literals in src/parallel/tests/executor.rs (now at lines ~1192, ~1355, ~1573, ~1783, ~1999); `cargo clippy --locked --all-targets --all-features -- -D warnings` finishes clean with no E0063, and `cargo fmt --all --check` plus `cargo test --lib` (2490 passed, 0 failed, 7 ignored) also pass.
