## Implementation Tasks

- [x] Make effective dependency-base selection and ref revision resolution authoritative for both dependency classification and analysis signatures; completion requires production code to hash the selected base ref rather than checkout `HEAD`, with a regression where only that ref advances. (verification: unit - `cargo test parallel::tests::unchanged_analysis_input --lib`; verification-id: analysis-suppression-liveness-tests)
- [x] Change unusable empty-order handling so queued work keeps the scheduler loop alive without recording a completed signature; completion requires fully drained execution to retain its existing termination behavior. (verification: integration - `cargo test parallel::tests --lib`; verification-id: analysis-suppression-liveness-tests)
- [x] Add a paused-time full scheduler-loop regression where queued work remains, no work is in flight, and the analyzer returns an empty order; completion requires the loop to survive the first result and invoke the analyzer again only at the next debounce-eligible evaluation. (verification: integration - `cargo test parallel::tests::unchanged_analysis_input --lib`; verification-id: analysis-suppression-liveness-tests)
- [x] Add bounded fail-open state for proposal-read and revision-resolution failures; completion requires no completed signature, no loop exit, and at most one timer-driven probe and analyzer attempt per ten-second cadence while failures persist, with explicit edges retaining one immediate attempt. (verification: integration - `cargo test parallel::tests::unchanged_analysis_input --lib`; verification-id: analysis-suppression-liveness-tests)
- [x] Add probe-recovery coverage where persistent signature failure later succeeds without a queue membership change; completion requires the next eligible evaluation to analyze once, record the usable signature, and suppress subsequent unchanged timer wakes. (verification: integration - `cargo test parallel::tests::unchanged_analysis_input --lib`; verification-id: analysis-suppression-liveness-tests)
- [x] Deterministically order queued and in-flight inputs before signature construction and analyzer/fallback consumption; completion requires order-only permutations to preserve one semantic input and real membership/content changes to continue invalidating the signature. (verification: unit - `cargo test parallel::tests::unchanged_analysis_input --lib parallel_run_service::tests`; verification-id: analysis-suppression-liveness-tests)
- [x] Schedule the first suppressed probe deadline when a completed signature is recorded and cap degraded deadlines at the five-minute expiry; completion requires no immediate 500 ms VCS/proposal probe after a healthy result and one degraded retry on the first eligible wake at or after exactly five paused-time minutes. (verification: integration - `cargo test parallel::tests::unchanged_analysis_input --lib`; verification-id: analysis-suppression-liveness-tests)
- [x] Run repository quality gates and all default tests; completion requires `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` to pass, with each new default-path timer test completing under one second. (verification: integration - `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`; verification-id: analysis-suppression-liveness-tests)

## Verification Evidence

Effective dependency-base authority (task 1):

- Unit: `src/parallel/tests/effective_dependency_base.rs` drives
  `ParallelExecutor::effective_dependency_base_evidence` and
  `probe_analysis_signature_materials` through a recording `WorkspaceManager` double. It asserts
  the selected base ref is the ref whose revision is resolved, that the probed signature material
  is `<base_ref>@<revision>`, and that `get_current_revision` (checkout `HEAD`) is never called —
  the test fails if `HEAD` is substituted. No VCS subprocess, clock, or repository state is used.
- Unit: `src/parallel/analysis_signature.rs` covers base-evidence changes invalidating the digest.
- Unit: `unchanged_analysis_input::effective_base_ref_change_rearms_analysis` covers both a
  re-pointed base ref and an advancing base ref through the scheduler gate with an injected probe.

Deterministic ordering (task 6):

- Unit: `unchanged_analysis_input::in_flight_order_permutations_produce_one_deterministic_analyzer_input`
  asserts an order-only permutation of the same in-flight set is one semantic signature and that
  the analyzer receives a sorted list, while a real membership change still re-arms analysis.
- Unit: `analysis_signature::identical_input_produces_equal_signature_regardless_of_iteration_order`
  (pre-existing) covers the signature side.
- The task text names `parallel_run_service::tests`; no such module exists in this repository, so
  analyzer/fallback consumption is covered where it is actually implemented
  (`ParallelExecutor::run_dependency_analysis_attempt`) by the tests above.

Loop liveness (tasks 2 and 3):

- Integration: `src/parallel/tests/analysis_liveness_loop.rs` runs the real
  `execute_with_order_based_reanalysis` loop under paused Tokio time against a temporary git
  repository. `empty_analysis_order_keeps_the_queued_scheduler_loop_alive_and_retries` proves the
  loop does not self-terminate on an empty order and retries at the ten-second cadence rather than
  the next 500 ms wake; `fully_drained_scheduler_still_terminates` proves the canonical drain exit
  is unchanged.

Bounded fail-open retry, probe recovery, and degraded expiry (tasks 4, 5, 7):

- Integration: `unchanged_analysis_input::persistent_signature_failure_is_rate_limited_across_timer_wakes`,
  `explicit_edge_bypasses_the_signature_failure_retry_deadline_once`,
  `signature_probe_recovery_reestablishes_suppression_without_a_queue_change`,
  `unusable_empty_result_retries_at_the_bounded_cadence`,
  `degraded_expiry_is_not_delayed_by_the_repository_probe_cadence`, and
  `suppressed_wakes_do_not_probe_before_the_bounded_deadline`.
- Unit: `analysis_signature::bounded_retry_blocks_only_until_the_debounce_cadence_elapses`,
  `healthy_record_probe_deadline_uses_the_bounded_repository_cadence`, and
  `degraded_record_probe_deadline_is_capped_at_its_expiry`.

Every new default-path test completes well under one second (slowest: 0.84 s for the full-loop
empty-order regression).

## Future Work

- Consider a separate cost-control policy for deterministic positive-capacity zero-dispatch results only if production evidence shows repeated erroneous analysis remains operationally significant; this proposal preserves the existing liveness-first canonical rule.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-analysis-suppression-liveness --archive-gate`
