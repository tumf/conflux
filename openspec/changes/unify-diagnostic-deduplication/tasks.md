## Implementation Tasks

- [x] Introduce `DiagnosticDeduplicationStore<K>` in `src/parallel/dedup.rs` (or equivalent) with generic key storage and `emit_or_suppress` behavior. (verification: unit - `cargo test parallel::dedup::tests::<new_store_emits_once_and_suppresses_duplicates>` proves first emit sends event and duplicate emits no event)
- [x] Replace the six diagnostic HashSet fields on `ParallelExecutor` with unified store fields and update `src/parallel/builder.rs`. (verification: unit - `cargo test parallel::tests::executor::<existing_diagnostic_tests>` compiles without manual HashSet initialization)
- [x] Migrate `emit_no_analysis_diagnostic`, `emit_capacity_zero_dispatch_diagnostic_once`, and `emit_analyze_failure_diagnostic_once` to thin wrappers around the unified store. (verification: integration - `cargo test parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` still observes one `dispatch_capacity_zero_after_analysis` log)
- [x] Update the 7 repeated executor-test helper initializations to use a single dedup store initialization. (verification: unit - `cargo test parallel::tests::executor::test_queue_notification_with_fresh_debounce_starts_analysis_after_initial_iteration parallel::tests::executor::test_persistent_idle_wait_does_not_poll_worktree_reconciliation_without_wake` compile and pass after removing repeated HashSet init blocks)
- [x] Preserve existing diagnostic behavior and suppression semantics for dependency blockers, queue reconciliation, no-analysis, analysis failure, capacity-zero, and TUI analysis-started logs. (verification: integration - run `cargo test parallel::tests::executor parallel::tests::manual_resolve tui::state::event_handlers::processing`)
- [x] Add/modify spec delta under `openspec/changes/unify-diagnostic-deduplication/specs/parallel-execution/spec.md`. (verification: unit - `cflx openspec validate unify-diagnostic-deduplication --strict` passes)

## Future Work

- Consider moving TUI-only dedup mechanisms into a UI-specific `DiagnosticDeduplicationStore` after scheduler-side consolidation lands.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-diagnostic-deduplication --archive-gate`
