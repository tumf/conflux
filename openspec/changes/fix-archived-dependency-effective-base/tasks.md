## Implementation Tasks

- [x] Define effective dependency base selection for archived dependency dispatch checks in the scheduler path. (verification: unit - focused `src/parallel/tests/executor.rs` test fails if `is_dependency_resolved()` always uses the original startup branch when an integration branch has advanced)
- [x] Preserve the archive-only safety guard while checking merge evidence against the selected effective dependency base. (verification: unit - existing or new `src/parallel/tests/executor.rs` regression keeps archived-but-not-merged dependencies blocked and fails if archive evidence alone unblocks dispatch)
- [x] Update dependency resolution diagnostics/status wording so archived-but-not-merged blockers identify the checked effective base and do not conflict with analysis/status `done` wording. (verification: unit - `src/parallel/tests/executor.rs` or related diagnostics assertions fail if an archived dependency is reported as dispatch-satisfied while still blocked without merge evidence)
- [x] Add regression coverage for stacked orchestration dependency unblocking after merge into the effective integration base. (verification: unit - `src/parallel/tests/executor.rs` constructs a dependency merged into the effective integration branch while the original branch lacks it, and asserts the dependent becomes dispatchable)
- [x] Run focused verification for dependency dispatch behavior. (verification: integration - `cargo test archived_dependency` passed; agent-exec job `a2c39fd2948af459a51a3b28cb0a5a2b`)
- [x] Run broader repository checks appropriate for changed Rust code. (verification: integration - `cargo test` passed; agent-exec job `0f9a984bccf61430116f07fcff914c36`)

## Future Work

- Consider a separate UX proposal if TUI/OpenSpec list output should expose `archived, waiting for merge into <base>` as a first-class dependency status.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-archived-dependency-effective-base --archive-gate`
