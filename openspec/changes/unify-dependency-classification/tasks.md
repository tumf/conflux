## Implementation Tasks

- [ ] Introduce `DependencyContext` in `src/parallel/dependency.rs` (or equivalent) that builds queued, in-flight, active, archived, rejected, and terminal-error lookup sets once. (verification: unit - `cargo test parallel::tests::executor::test_single_queued_archived_dependency_waits_until_merged parallel::tests::executor::test_single_queued_archived_dependency_can_dispatch_after_merge` pass after context extraction)
- [ ] Move `effective_dependency_base` selection into `DependencyContext` or a directly-owned helper used by it. (verification: integration - `cargo test parallel::tests::executor::test_archived_dependency_uses_effective_integration_base_after_startup` still proves current integration branch unblocks archived dependencies)
- [ ] Refactor `classify_queued_work` to call `DependencyContext` instead of rebuilding HashSets and reimplementing dependency loops. (verification: unit - `cargo test parallel::tests::executor::test_blocked_only_reanalysis_skips_analyzer_for_merge_wait_and_terminal_error` still passes)
- [ ] Refactor `select_changes_for_dispatch` to call `DependencyContext` for dependency classification and blocker fingerprinting. (verification: integration - `cargo test parallel::tests::executor::test_single_queued_archived_dependency_waits_until_merged parallel::tests::executor::test_single_queued_archived_dependency_can_dispatch_after_merge` pass)
- [ ] Verify no behavior change for zero-capacity and manual resolve flows. (verification: integration - `cargo test parallel::tests::manual_resolve::test_manual_resolve_zero_capacity_runs_analysis_but_suppresses_apply_dispatch parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` pass)
- [ ] Add spec delta under `openspec/changes/unify-dependency-classification/specs/parallel-execution/spec.md`. (verification: unit - `cflx openspec validate unify-dependency-classification --strict` passes)

## Future Work

- Consider moving dependency blocker diagnostic formatting into `DependencyContext` after behavior is stabilized.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-dependency-classification --archive-gate`
