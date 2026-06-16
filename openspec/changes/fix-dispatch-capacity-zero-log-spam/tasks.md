## Implementation Tasks

- [ ] Add a dedicated in-memory deduplication guard for the capacity-zero dispatch diagnostic in `src/parallel/queue_state.rs` (near `emit_no_analysis_diagnostic` and the `available_slots == 0` branch). (verification: unit - add `cargo test parallel::tests::executor::<new_capacity_zero_dedup_helper_test>` or equivalent that exercises the guard and proves identical keys are suppressed while changed keys emit)
- [ ] Modify the `available_slots == 0` path (`queue_state.rs:2617-2642`) to emit the "Dispatch suppressed after dependency analysis" log via the new guard instead of unconditionally. (verification: integration - run `cargo test parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` and assert `saw_capacity_diagnostic` remains true)
- [ ] Add a regression test under `src/parallel/tests/manual_resolve.rs` (or `executor.rs`) that forces repeated re-analysis under constant zero-capacity and asserts the capacity-zero dispatch log appears at most once for an unchanged signature. (verification: integration - `cargo test parallel::tests::manual_resolve::<new_repeated_capacity_zero_does_not_spam_dispatch_diagnostic>` confirms the capacity-zero log appears once for unchanged key across ≥2 iterations)
- [ ] Add a minimal `MODIFIED Requirements` delta under `openspec/changes/fix-dispatch-capacity-zero-log-spam/specs/parallel-execution/spec.md` that extends the existing canonical "Dependency-blocked diagnostics are stable and non-spamming" requirement (see `openspec/specs/parallel-execution/spec.md:157`) to cover dispatch-capacity-zero diagnostics. (verification: unit - `cflx openspec validate fix-dispatch-capacity-zero-log-spam --strict` passes with the exact canonical heading copied)
- [ ] Run targeted Rust tests and repository quality gates. (verification: manual - run `cargo test parallel::tests::executor parallel::tests::manual_resolve`; then run discovered default lint/typecheck/test commands excluding heavy tests)

## Future Work

- Optional TUI-level idempotency hardening for any other operator-visible capacity or scheduler logs (out of scope for this minimal fix).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-dispatch-capacity-zero-log-spam --archive-gate`
