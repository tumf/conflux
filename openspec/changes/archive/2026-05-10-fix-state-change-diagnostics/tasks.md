## Implementation Tasks

- [x] Introduce dependency blocker fingerprints in the scheduler path that emits `DependencyBlocked` diagnostics/events, using comparable state that includes change id, unresolved dependency ids, and dependency target classes. (verification: unit - added tests in `src/parallel/tests/executor.rs`; targeted cargo test will be run before final completion)
- [x] Emit a new dependency blocker diagnostic/event only when a previously unseen blocker snapshot appears or an existing blocked change's blocker snapshot changes. (verification: unit - added `test_changed_dependency_blocker_snapshot_emits_again` in `src/parallel/tests/executor.rs`; targeted cargo test will be run before final completion)
- [x] Emit `DependencyResolved` once when a previously blocked change becomes unblocked, then clear the remembered blocker state so later loops do not re-emit resolution unless a new blocked transition occurs. (verification: unit - added `test_dependency_resolved_emits_once_and_blocked_again_can_emit` in `src/parallel/tests/executor.rs`; targeted cargo test will be run before final completion)
- [x] Keep scheduler suppression state non-authoritative and in-memory only, with scheduling decisions still derived from analysis, workspace, and git state. (verification: unit - added `test_dependency_suppression_state_does_not_change_dispatch_selection` in `src/parallel/tests/executor.rs`; manual inspection confirms state is an in-memory `HashMap` on `ParallelExecutor` and is only used around diagnostic/event emission and worktree recreation after derived resolution)
- [x] Harden reducer/TUI handling so duplicate `DependencyBlocked` and `DependencyResolved` events do not append duplicate user-visible logs when the display state did not transition. (verification: unit - added `test_duplicate_dependency_events_are_tui_log_noops` in `src/tui/state.rs`; targeted cargo test will be run before final completion)
- [x] Bound repeated worktree or merge-wait diagnostics derived from unchanged observations by state-change detection, rate limiting, or summary behavior, without using that suppression for workflow control. (verification: integration - existing repository tests `tui::state::event_handlers::errors::duplicate_merge_deferred_warning_is_suppressed` and `distinct_merge_deferred_reason_is_logged_after_suppressed_duplicate` passed under `cargo test tui::state --lib`; queue/worktree reconciliation diagnostics are also in-memory deduped via `queue_reconciliation_diagnostics_seen` and `no_analysis_diagnostics_seen`)
- [x] Run default Rust verification for the touched modules. (verification: integration - ran `cargo test dependency --lib`, `cargo test tui::state --lib`, `cargo check --lib`, `cargo fmt`, `cflx openspec validate fix-state-change-diagnostics --strict`, and `cflx openspec validate fix-state-change-diagnostics --archive-gate`)

## Future Work

- Long-running real TUI run observation can be used after implementation to compare log growth under repeated unchanged scheduler loops, but the implementation must be accepted based on repository-verifiable tests and bounded manual evidence rather than waiting for production-like runtime duration.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-state-change-diagnostics --archive-gate`
