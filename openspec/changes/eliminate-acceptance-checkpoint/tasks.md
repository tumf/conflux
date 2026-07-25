## Implementation Tasks

- [ ] 1. Remove the JSON checkpoint model and filesystem lifecycle while preserving only the independently required `APPLY_BLOCKED/marker.md` parsing, writing, and consumption behavior (verification: unit - `cargo test parallel::acceptance_state` and `rg 'acceptance-state\.json|AcceptanceStateStatus|load_acceptance_state|save_acceptance_state' src` show no production checkpoint implementation)
- [ ] 2. Replace serial runtime checkpoint reads/writes with in-memory acceptance-attempt context for the active run and repository-visible follow-up/blocked evidence for durable outcomes (verification: integration - `cargo test serial_run_service` covers uninterrupted PASS-to-archive, FAIL-to-apply, stalled marker creation, and restart acceptance rerun)
- [ ] 3. Replace parallel dispatch checkpoint reads/writes with in-memory cycle context, routing complete unarchived work to acceptance after restart and incomplete tasks to apply (verification: integration - `cargo test parallel::dispatch` and focused tests in `src/parallel/tests/executor.rs` cover Applied, Archiving, incomplete-task, and archived/base-integrated resume paths)
- [ ] 4. Define restart semantics for pre-stall retry context: do not reconstruct retry count, prior finding identities, or semantic baseline from a generated checkpoint; rerun acceptance safely and persist only normal `tasks.md` follow-up or a final resumable stalled marker (verification: integration - focused restart tests in `src/serial_run_service.rs` and `src/parallel/tests/executor.rs` prove missing in-memory context cannot skip acceptance and marker-backed stalled work remains blocked)
- [ ] 5. Delete checkpoint-specific cleanup and Git handling from archive, merge, queue, execution-state, semantic fingerprint, and workspace cleanup paths without weakening unrelated dirty-state checks (verification: integration - `cargo test execution::archive parallel::merge orchestration::acceptance` and `rg 'acceptance-state\.json|delete_acceptance_state' src`)
- [ ] 6. Add an end-to-end regression fixture for `apply complete -> acceptance PASS -> archive commit -> post-archive merge` that asserts no `.cflx/acceptance-state.json` is created, no cleanup dirties the worktree, and no manual `MergeDeferred(auto_resumable=false)` occurs (verification: e2e - focused non-heavy test in `src/parallel/tests/executor.rs` reaches resolving/merged and inspects `git status --porcelain`)
- [ ] 7. Preserve negative behavior for genuine blockers: unrelated dirty files, surviving active change directory, missing archive entry, and invalid nested archive layout still defer or fail with concrete evidence (verification: integration - focused tests in `src/parallel/merge.rs` and `src/execution/archive.rs` pass)
- [ ] 8. Remove obsolete `.gitignore` and tests/documentation that exist only for `.cflx/acceptance-state.json`, while retaining ignore entries and runtime artifacts with independent purposes (verification: unit - `git diff -- .gitignore` plus `rg 'acceptance-state\.json' --glob '!openspec/changes/**'` returns no runtime/config references)
- [ ] 9. Run repository quality gates and keep default tests under the project heavy-test policy (verification: integration - `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` pass; tests exceeding one second are optimized or marked heavy)

## Future Work

- Historical repositories may retain the path in old commits; this change does not rewrite published Git history.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate eliminate-acceptance-checkpoint --archive-gate`
