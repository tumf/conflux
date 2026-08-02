## Implementation Tasks

- [ ] Gate worktree-only archived-dirty discovery in `src/parallel/queue_state.rs` against `OrchestratorState::initial_change_ids()` before adding scheduler-local queued work, while leaving reducer-queued IDs and reducer-owned resolve/reject waiters unchanged. (verification: integration - `cargo test parallel::tests::executor` proves a recoverable unselected worktree produces no queued/repair addition and no execution diagnostic; verification-id: run-admission-recovery-tests)

- [ ] Preserve explicit recovery admission through both entry paths: initial TUI selected IDs in `src/tui/orchestrator.rs` and Running-mode queue additions through `OrchestratorState::add_dynamic_change`; do not add a second allowlist or durable state. (verification: unit/integration - `cargo test tui::orchestrator orchestration::state parallel::tests::executor` proves exact startup membership and dynamic membership growth; verification-id: run-admission-recovery-tests)

- [ ] Add a temporary-Git regression fixture with selected `fresh` and unselected archived-dirty `stale`; assert queue reconciliation, dependency analysis candidates, and lifecycle events never include `stale`, and assert its worktree status and revision remain unchanged. (verification: integration - `cargo test parallel::tests::executor` fails if the unselected workspace is queued, analyzed, committed, archived, or merged; verification-id: run-admission-recovery-tests)

- [ ] Add the positive counterpart proving the same archived-dirty workspace becomes recoverable after initial selection or explicit Running-mode queue admission and resumes archive finalization/archive-complete handoff without rerunning apply. (verification: integration - `cargo test parallel::tests::executor` asserts recovery addition and workspace-derived phase routing only after admission; verification-id: run-admission-recovery-tests)

- [ ] Preserve manual merge ownership and terminal stop gates with regression assertions for `MergeWait` requiring accepted `ResolveMerge`, empty ordinary queue `ResolveWait` startup, already-merged worktree residue, and terminal-error explicit retry. (verification: integration - `cargo test parallel::tests::executor && cargo test tui::orchestrator` covers manual retry, zero-queue lane wait, merged residue, and terminal-error fixtures; verification-id: run-admission-recovery-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-unselected-worktree-recovery --archive-gate`

The implementation must also pass `cargo test parallel::tests::executor && cargo test tui::orchestrator && cargo test tui::state`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Future Work

- A separate operator workflow may later expose interrupted unselected worktrees as explicit attention items, but it must not auto-admit them to execution.
