## Implementation Tasks

- [ ] 1.1 Emit the existing AnalysisCompleted once from the real analysis producer immediately after a returned analyzer outcome, before every empty-order/capacity path, preserving the returned outcome and existing scheduling semantics (verification: integration - cargo test --lib analysis_activity_completion; verification-id: producer-completion)
- [ ] 1.2 Add deterministic regression coverage of pending-to-return, healthy/metadata/fallback/empty results, legacy groups count, real dispatcher/facts closure, unrelated active-work retention and no completion on interrupted pending analysis; completion requires nonempty tests that fail if the emission is removed (verification: integration - src/parallel/tests/analysis_activity_completion.rs; verification-id: producer-completion)
- [ ] 1.3 Preserve empty-order retry and unchanged-input suppression decisions, with the real pass/loop reaching its existing disposition after activity closure rather than synthetic terminal events (verification: integration - src/parallel/tests/analysis_liveness_loop.rs and src/parallel/tests/unchanged_analysis_input.rs; verification-id: analysis-policy-controls)
- [ ] 1.4 Keep existing execution-observation activity and phase contracts unchanged while wiring actual producer completion into that same dispatch boundary (verification: integration - src/orchestration/execution_facts/tests.rs; verification-id: execution-observation-controls)

## Final Validation

Archive validation owns final OpenSpec validation: `cflx openspec validate fix-analysis-activity-completion --archive-gate`. Each declared verification is one direct bounded run with selected-test counts recorded; proposal validation alone cannot complete an implementation task.

## Future Work

Parent observes a fresh integrated-revision isolated owner through native metadata-only analysis and synthetic lifecycle completion. This does not certify a deployment or real AI quality. No original owner replacement or candidate modification is part of the change.
