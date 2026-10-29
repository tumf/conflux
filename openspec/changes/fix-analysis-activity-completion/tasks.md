## Implementation Tasks

- [x] 1.1 Emit the existing AnalysisCompleted once from the real analysis producer immediately after a returned analyzer outcome, before every empty-order/capacity path, preserving the returned outcome and existing scheduling semantics (verification: integration - cargo test --lib analysis_activity_completion; verification-id: producer-completion)
- [x] 1.2 Add deterministic regression coverage of pending-to-return, healthy/metadata/fallback/empty results, legacy groups count, real dispatcher/facts closure, unrelated active-work retention and no completion on interrupted pending analysis; completion requires nonempty tests that fail if the emission is removed (verification: integration - src/parallel/tests/analysis_activity_completion.rs; verification-id: producer-completion)
- [x] 1.3 Preserve empty-order retry and scheduler liveness, with the real pass/loop reaching its existing disposition after activity closure rather than synthetic terminal events (verification: integration - src/parallel/tests/analysis_liveness_loop.rs; verification-id: analysis-policy-controls)
- [x] 1.4 Preserve unchanged-input suppression and provenance classification after returned analysis activity closes, with no new analyzer or workflow-control authority (verification: integration - src/parallel/tests/unchanged_analysis_input.rs; verification-id: analysis-input-controls)
- [x] 1.5 Keep existing execution-observation activity and phase contracts unchanged while wiring actual producer completion into that same dispatch boundary (verification: integration - src/orchestration/execution_facts/tests.rs; verification-id: execution-observation-controls)

## Notes

- evidence (producer-completion): `cargo test --locked --offline --lib --features web-monitoring analysis_activity_completion` -> 7 passed, 0 ignored. With the `AnalysisCompleted` emission removed from `run_dependency_analysis_attempt`, 6 of 7 fail; the interrupted-pending test passes either way by design (it proves no fabricated completion).
- evidence (analysis-policy-controls): `analysis_liveness_loop` -> 2 passed.
- evidence (analysis-input-controls): `unchanged_analysis_input` -> 27 passed.
- evidence (execution-observation-controls): `agent_execution_observability` -> 46 passed, 3 ignored (pre-existing heavy tests, not part of this change).
- The regression harness uses an in-flight dependency wait to keep dispatch inert. A missing-dependency fixture was rejected because its diagnostic is a process-level `Error`, which clears every facts-store activity and would mask a missing completion. Every new test asserts that no `AllCompleted`, `Stopped` or `Error` was dispatched.
- `cargo clippy --all-targets --features web-monitoring -D warnings` and `cargo fmt --check` are clean. In the full `--lib` suite, `vcs::git::commands::status_policy::...::production_status_paths_preserve_conflicted_classification` fails identically on unmodified HEAD (git 2.53.0), so it is pre-existing and outside this change. `test_worktree_remove_runs_teardown_before_removal` failed once under full-suite load and passed in isolation.

## Final Validation

Archive validation owns final OpenSpec validation: `cflx openspec validate fix-analysis-activity-completion --archive-gate`. Each declared verification is one direct bounded run with selected-test counts recorded; proposal validation alone cannot complete an implementation task.

## Future Work

Parent observes a fresh integrated-revision isolated owner through native metadata-only analysis and synthetic lifecycle completion. This does not certify a deployment or real AI quality. No original owner replacement or candidate modification is part of the change.
