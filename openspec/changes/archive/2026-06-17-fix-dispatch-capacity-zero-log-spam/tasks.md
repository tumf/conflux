## Implementation Tasks

- [x] Add a dedicated in-memory deduplication guard for the capacity-zero dispatch diagnostic in `src/parallel/queue_state.rs` (near `emit_no_analysis_diagnostic` and the `available_slots == 0` branch). (verification: unit - `cargo test parallel::tests::executor::capacity_zero_dispatch_diagnostic_guard_suppresses_identical_keys_and_emits_changed_keys` passed via agent-exec job `9764ef3fc2eddbf848b8d29478227356`)
- [x] Modify the `available_slots == 0` path (`queue_state.rs:2617-2642`) to emit the "Dispatch suppressed after dependency analysis" log via the new guard instead of unconditionally. (verification: integration - `cargo test parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` passed via agent-exec job `7b265ffd87f5ee045fe49238e69a4d2f`, preserving `saw_capacity_diagnostic`)
- [x] Add a regression test under `src/parallel/tests/manual_resolve.rs` (or `executor.rs`) that forces repeated re-analysis under constant zero-capacity and asserts the capacity-zero dispatch log appears at most once for an unchanged signature. (verification: integration - `cargo test parallel::tests::manual_resolve::repeated_capacity_zero_does_not_spam_dispatch_diagnostic` passed via agent-exec job `f0cfd69950af7e1b44cff45f605aff02`, confirming one capacity-zero log across two analysis iterations)
- [x] Add a minimal `MODIFIED Requirements` delta under `openspec/changes/fix-dispatch-capacity-zero-log-spam/specs/parallel-execution/spec.md` that extends the existing canonical "Dependency-blocked diagnostics are stable and non-spamming" requirement (see `openspec/specs/parallel-execution/spec.md:157`) to cover dispatch-capacity-zero diagnostics. (verification: manual - inspected `openspec/changes/fix-dispatch-capacity-zero-log-spam/specs/parallel-execution/spec.md` for the exact canonical heading and capacity-zero scenarios; runnable command `cflx openspec validate fix-dispatch-capacity-zero-log-spam --strict` passed)
- [x] Run targeted Rust tests and repository quality gates. (verification: manual - `cargo fmt` passed; targeted modules passed via `cargo test parallel::tests::executor` job `8995866e5015c3d3d29eb92602ebc71c` and `cargo test parallel::tests::manual_resolve` job `15c294124f910c917ee47612669740bd`; default gates passed via `cargo clippy -- -D warnings` job `f7d746b879637ce4e20072bdbca01601` and `cargo test` job `ccf2582fc10b5e028c4f295cf493a785`)

## Future Work

- Optional TUI-level idempotency hardening for any other operator-visible capacity or scheduler logs (out of scope for this minimal fix).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-dispatch-capacity-zero-log-spam --archive-gate`
