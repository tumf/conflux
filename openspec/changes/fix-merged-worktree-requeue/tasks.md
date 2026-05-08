## Implementation Tasks

- [ ] Gate archived-dirty reconciliation against terminal merged worktrees. Completion condition: before `src/parallel/queue_state.rs` inserts an archived-dirty repair candidate, it proves from workspace-local Git/base-branch evidence that the change is not already merged. (verification: unit - add a targeted test in `src/parallel/tests/executor.rs` covering `state=Merged` or equivalent base-tree evidence, then run the exact new test)

- [ ] Preserve legitimate archived-dirty repair for non-merged interrupted archive work. Completion condition: an archived workspace whose archive move is complete but merge has not completed is still eligible for archive-complete handoff or repair, and the existing archived-dirty discovery test continues to pass. (verification: unit - keep/update `src/parallel/tests/executor.rs::test_archived_dirty_reconciliation_discovers_workspace_after_archive_failed_terminal_state` or a replacement test and run `cargo test test_archived_dirty_reconciliation_discovers_workspace_after_archive_failed_terminal_state`)

- [ ] Prevent terminal merged worktrees from re-entering apply or acceptance. Completion condition: a requeued or reconciled worktree detected as merged returns terminal/no-op behavior and cannot emit `ApplyStarted` or `AcceptanceStarted` for that change. (verification: integration - add an event-level executor test in `src/parallel/tests/executor.rs` asserting no apply/acceptance events are emitted for the merged leftover worktree path)

- [ ] Keep reducer and scheduler intent semantics intact. Completion condition: reducer-owned `ResolveWait` and manual `MergeWait` behavior remain unchanged, and queue reconciliation still handles reducer-visible queued candidates that are active and not terminal. (verification: integration - run existing resolve/merge-wait scheduler tests in `src/parallel/tests/executor.rs`, including tests matching `resolve_wait`, `manual_resolve`, or `merge_wait`)

- [ ] Run repository verification. Completion condition: targeted regression tests pass, default test suite passes or long-running tests are correctly marked heavy, and lint/typecheck succeeds or blockers are recorded with exact command output. (verification: integration - run `cargo test <new_test_name>`, relevant existing targeted tests, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings` if available)

## Future Work

- Consider a separate cleanup-focused proposal if terminal merged worktrees should be proactively pruned sooner rather than merely ignored by reconciliation.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate. Expected archive gate: `cflx openspec validate fix-merged-worktree-requeue --archive-gate`
