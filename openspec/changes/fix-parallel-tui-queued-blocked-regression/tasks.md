## Implementation Tasks

- [ ] 1. Route TUI `F5` start/resume/retry through reducer-owned queue intent in `src/tui/state.rs` and `src/tui/command_handlers.rs`, removing the remaining local-only queued mutation path for parallel startup (verification: `cargo test test_start_processing_syncs_reducer_queue_intent test_resume_processing_syncs_reducer_queue_intent`)
- [ ] 2. Preserve queued startup rows across the initial parallel `ChangesRefreshed` flow in `src/tui/orchestrator.rs` and `src/tui/runner.rs`, so startup refresh cannot regress selected rows to `not queued` before execution begins (verification: `cargo test test_parallel_start_refresh_preserves_queued_rows`)
- [ ] 3. Keep dependency block/resolution transitions reducer-owned in `src/orchestration/state.rs`, `src/parallel/queue_state.rs`, and `src/tui/state.rs`, preserving queued intent while `blocked` is displayed and restoring `queued` on resolution (verification: `cargo test test_dependency_block_preserves_queued_intent test_dependency_resolved_restores_queued_display`)
- [ ] 4. Restrict startup rejection and safety-net reset logic to only the backend-rejected change IDs so unrelated queued/blocked rows are not cleared in `src/tui/state.rs` and any affected startup path (verification: `cargo test test_parallel_start_rejected_does_not_clear_other_rows test_parallel_start_rejected_clears_queued_status`)
- [ ] 5. Add targeted regression coverage in `src/tui/state.rs`, `src/orchestration/state.rs`, and any required integration tests for select-mode `F5`, stopped-mode `F5`, dependency blocked/resolved, and parallel startup refresh (verification: `cargo test test_parallel_start_refresh_preserves_queued_rows test_dependency_block_preserves_queued_intent test_resume_processing_syncs_reducer_queue_intent`)

## Future Work

- Manually verify the fix in an interactive TUI session with a known dependency chain after implementation.
- Audit remote/Web queue-intent synchronization separately if similar local-only queue writes remain outside the parallel TUI path.
