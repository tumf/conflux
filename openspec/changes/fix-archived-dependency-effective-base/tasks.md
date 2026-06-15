## Implementation Tasks

- [ ] Define effective dependency base selection for archived dependency dispatch checks in the scheduler path. (verification: unit - focused `src/parallel/tests/executor.rs` test fails if `is_dependency_resolved()` always uses the original startup branch when an integration branch has advanced)
- [ ] Preserve the archive-only safety guard while checking merge evidence against the selected effective dependency base. (verification: unit - existing or new `src/parallel/tests/executor.rs` regression keeps archived-but-not-merged dependencies blocked and fails if archive evidence alone unblocks dispatch)
- [ ] Update dependency resolution diagnostics/status wording so archived-but-not-merged blockers identify the checked effective base and do not conflict with analysis/status `done` wording. (verification: unit - `src/parallel/tests/executor.rs` or related diagnostics assertions fail if an archived dependency is reported as dispatch-satisfied while still blocked without merge evidence)
- [ ] Add regression coverage for stacked orchestration dependency unblocking after merge into the effective integration base. (verification: unit - `src/parallel/tests/executor.rs` constructs a dependency merged into the effective integration branch while the original branch lacks it, and asserts the dependent becomes dispatchable)
- [ ] Run focused verification for dependency dispatch behavior. (verification: integration - `cargo test dependency` or a narrower documented set covering `fix-dependency-dispatch-after-merge` and the new effective-base regression passes)
- [ ] Run broader repository checks appropriate for changed Rust code. (verification: integration - `cargo test` or documented project check command passes, or failures are documented with unrelated evidence)

## Future Work

- Consider a separate UX proposal if TUI/OpenSpec list output should expose `archived, waiting for merge into <base>` as a first-class dependency status.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-archived-dependency-effective-base --archive-gate`
