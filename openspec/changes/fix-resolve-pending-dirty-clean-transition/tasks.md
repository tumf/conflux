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
- [x] `cargo fmt --check` が commit-path/archive readiness をブロックします。実行コマンド: `agent-exec run -- cargo fmt --check`（job_id: 930a06e6e7c8fe02e409994ecd1caa12）。失敗箇所は `src/orchestration/state.rs:3551`, `src/orchestration/state.rs:3564`, `src/orchestration/state.rs:3602` のテスト追加部で、rustfmt が複数行フォーマット差分を要求しています。作業ツリー自体は `git status --short` で clean でしたが、フォーマットチェックが失敗するため最終 archive commit 前に `cargo fmt` 相当の整形を反映する必要があります。（解決: 指摘箇所を rustfmt 期待形式へ整形し、`agent-exec run -- cargo fmt --check` で再検証）
