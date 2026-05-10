## Implementation Tasks

- [ ] Thread the target repository root into `ParallelizationAnalyzer` dependency-target evidence collection. (verification: unit - analyzer tests in `src/analyzer.rs` assert archived/rejected collectors read from an explicit temp repo root rather than process cwd)
- [ ] Update analyzer construction call sites to pass the same repo root used by Conflux execution. (verification: integration - test or compile-visible constructor update covers `src/parallel_run_service.rs` and any direct analyzer creation paths)
- [ ] Preserve fail-closed diagnostics for truly missing and rejected dependencies. (verification: unit - add or update `src/analyzer.rs` tests that assert `Missing dependency reference` and `Rejected dependency reference` remain errors with target classifications)
- [ ] Add a cwd-independence regression test for archived dependencies. (verification: integration - add a test in `src/analyzer.rs` or `src/parallel/tests/executor.rs` that creates `openspec/changes/archive/<date>-dep/proposal.md`, changes process cwd or instantiates from another cwd, and asserts a change depending on `dep` validates without missing-dependency error)
- [ ] Verify focused and relevant regression tests pass. (verification: unit - run `cargo test analyzer`; integration - run affected `cargo test` names in `src/parallel/tests/executor.rs` if constructor signatures require broader updates)

## Final Validation

Expected archive gate: `cflx openspec validate fix-analysis-archive-root --archive-gate`
