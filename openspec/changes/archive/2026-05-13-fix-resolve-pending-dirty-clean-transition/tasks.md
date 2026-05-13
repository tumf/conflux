## Implementation Tasks

- [x] Confirm reducer demotion from `ResolveWait` to `MergeWait` for manual dirty-base deferral. (verification: unit - add/update `src/orchestration/state.rs` tests applying `ReducerCommand::ResolveMerge` followed by `ExecutionEvent::MergeDeferred { auto_resumable: false, .. }`; completion condition: display status is `merge wait`, `resolve_wait_change_ids()` no longer includes the change, and reducer invariants hold)
- [x] Confirm reducer promotion of exactly one clean `ResolveWait` waiter into `Resolving`. (verification: unit - add/update `src/orchestration/state.rs` tests for `promote_next_base_mutating_lane_waiter`; completion condition: one waiter becomes `resolving`, remaining waiters stay `resolve pending`, and global invariants hold)
- [x] Wire scheduler retry evaluation so dirty-to-clean base repository state progresses pending `ResolveWait` work without another `M` keypress. (verification: integration - add/update `src/parallel/tests/executor.rs` or focused queue-state tests that simulate dirty base deferral, clean the base, trigger scheduler evaluation, and observe retry dispatch; completion condition: the pending change transitions from `resolve pending` to `resolving` or completes merge through scheduler-owned execution)
- [x] Preserve dirty-base manual demotion when no active `Resolving` or `Rejecting` lane exists. (verification: integration - add/update `src/parallel/tests/executor.rs` coverage for a dirty base with no lane occupant; completion condition: emitted `MergeDeferred` has `auto_resumable=false`, reducer display becomes `merge wait`, and no retry loop keeps the row as `resolve pending`)
- [x] Preserve auto-resumable waiting when another active `Resolving` or `Rejecting` lane blocks the retry. (verification: integration - run or extend existing active resolving/rejecting deferral tests in `src/parallel/tests/executor.rs` and `src/tui/orchestrator.rs`; completion condition: deferred change remains `resolve pending` until the lane clears, then exactly one retry is promoted)
- [x] Keep `ChangesRefreshed` reconciliation from regressing `ResolveWait` without concrete deferred evidence. (verification: unit - run or extend TUI/reducer tests in `src/tui/state.rs` and `src/orchestration/state.rs`; completion condition: workspace archived observations alone preserve `resolve pending`, while explicit `MergeDeferred(auto_resumable=false)` demotes to `merge wait`)
- [x] Ensure TUI display sync reflects reducer-owned demotion and promotion states. (verification: unit - add/update `src/tui/state.rs` tests around `apply_display_statuses_from_reducer` and resolve completion/promotion paths; completion condition: TUI rows show `merge wait`, `resolve pending`, or `resolving` exactly as reducer snapshots report)
- [x] Verify no out-of-worktree durable state becomes authoritative for retry routing. (verification: manual - inspect changed code paths in `src/orchestration/state.rs`, `src/parallel/queue_state.rs`, `src/parallel/merge.rs`, and `src/tui/orchestrator.rs`; completion condition: retry decisions derive from workspace file state, workspace git state, base-branch comparison, and in-memory scheduler state only, not logs or `~/.local/state/cflx/**`)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-resolve-pending-dirty-clean-transition --archive-gate`

Implementation validation should include these commands:

```bash
cargo test orchestration::state
cargo test parallel::tests::executor
cargo test tui::state
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Acceptance #1 Failure Follow-up
- [x] `cargo fmt --check` が commit-path/archive readiness をブロックします。実行コマンド: `agent-exec run -- cargo fmt --check`（job_id: 930a06e6e7c8fe02e409994ecd1caa12）。失敗箇所は `src/orchestration/state.rs:3551`, `src/orchestration/state.rs:3564`, `src/orchestration/state.rs:3602` のテスト追加部で、rustfmt が複数行フォーマット差分を要求しています。作業ツリー自体は `git status --short` で clean でしたが、フォーマットチェックが失敗するため最終 archive commit 前に `cargo fmt` 相当の整形を反映する必要があります。(verification: manual - repository evidence is source path `src/orchestration/state.rs` test formatting around lines 3551, 3564, and 3602; runnable command `cargo fmt --check` passed via `agent-exec run -- cargo fmt --check` job_id: 6028f2859945bd93e98b9835803b7840)

## Acceptance #4 Failure Follow-up
- [x] Correct task metadata that caused repository-verifiable evidence errors at `tasks.md:28` and self-referential final-validation checkbox errors at `tasks.md:35` and `tasks.md:38`; historical final-validation records were converted to non-checkbox Notes and the formatter follow-up now carries accepted ownership plus source-path evidence. (verification: not-testable - metadata-only correction in `openspec/changes/fix-resolve-pending-dirty-clean-transition/tasks.md`; validator implementation reference is `src/openspec_cmd/validation.rs`; no runtime code path changed.)

## Notes

Acceptance #2 observed archive gate validation blocking the final archive commit path. Command: `agent-exec run -- cflx openspec validate fix-resolve-pending-dirty-clean-transition --archive-gate` (job_id: db723ff6d29eba4880449c96adbe11d5) exited with code 1. Failure: `openspec/changes/fix-resolve-pending-dirty-clean-transition/tasks.md:28: Behavior-bearing task missing '(verification: ...)' note`. Resolution: this non-behavioral validation note is recorded outside checkbox task sections because archive validation is the authoritative final OpenSpec validation gate. Previous `cargo fmt --check` blocker is resolved: `agent-exec run -- cargo fmt --check` (job_id: 6028f2859945bd93e98b9835803b7840) passed, and `git status --short` was clean at the time of the acceptance finding.

Acceptance #3 observed that the checked follow-up task describing the `cargo fmt --check` fix was missing an explicit `(verification: ...)` note. That historical record is intentionally non-checkbox narrative because final OpenSpec validation is already owned by the non-checkbox `## Final Validation` section above.

Acceptance #4 observed that the Acceptance #1 follow-up verification note lacked accepted ownership/source-path evidence and that the Acceptance #3/Acceptance #4 follow-up checkboxes were being interpreted as self-referential final-validation tasks. The active follow-up above resolves this as metadata-only work: `tasks.md:28` now cites `src/orchestration/state.rs` and `cargo fmt --check`, while the historical final-validation command records remain in this excluded Notes section.
