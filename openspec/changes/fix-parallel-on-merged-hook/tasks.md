## 1. Implementation
- [x] 1.1 `ParallelExecutor::execute_apply_and_archive_parallel` の`MergeAttempt::Merged`成功時に`HookRunner::run_hook(HookType::OnMerged, ...)`を追加する（検証: `src/parallel/mod.rs`のマージ成功ブロックにフック呼び出しがあること）。
- [x] 1.2 `ParallelExecutor::execute_with_order_based_reanalysis`のマージ成功経路で同様の`on_merged`呼び出しを追加する（検証: `src/parallel/mod.rs`の`attempt_merge`成功分岐にフック呼び出しがあること）。
- [x] 1.3 `HookContext`でchange/task進捗と`workspace_path`を設定し、取得失敗時は0/0を維持する（検証: `HookContext`の`with_change`と`with_parallel_context`が使われていること）。

## 2. Validation
- [x] 2.1 `hooks.on_merged`を設定した状態でパラレル実行のマージ成功時にフックが走ることを確認する（検証: 手動実行ログまたは追加したテストで`on_merged`実行が確認できること）。

## 3. Acceptance #1 Failure Follow-up
- [x] 3.1 Git作業ツリーがdirtyのため、`Cargo.lock`、`openspec/changes/fix-parallel-on-merged-hook/tasks.md`、`src/parallel/mod.rs`、`src/parallel/tests/executor.rs`の未コミット変更を解消する。
- [x] 3.2 `src/parallel/mod.rs: merge_and_resolve()` が `on_merged` を実行する一方で `execute_with_order_based_reanalysis()` や `execute_apply_and_archive_parallel()` の `attempt_merge()` 成功分岐でも `on_merged` を実行しており二重実行になるため、成功時に1回だけ実行されるよう整理する。
- [x] 3.3 `src/parallel/mod.rs: merge_and_resolve()` の `on_merged` 用 `HookContext` に `with_parallel_context` がなく `OPENSPEC_WORKSPACE_PATH` が設定されないため、parallel mode の HookContext 要件を満たすよう workspace_path を渡す。
- [x] 3.4 `src/tui/runner.rs` の手動マージ成功時 `on_merged` HookContext に `with_parallel_context` がなく `OPENSPEC_WORKSPACE_PATH` が設定されないため、worktree path を渡す。

## 4. Acceptance #2 Failure Follow-up
- [x] 4.1 `src/parallel_run_service.rs` の `create_executor_with_queue_state()` と `run_parallel()` が `ParallelExecutor::set_hooks` を呼ばず `self.hooks` が常に `None` になるため、`src/parallel/mod.rs` の自動マージ成功経路で `on_merged` が実行されない。HookRunner を生成して executor に設定する。

## 5. Acceptance #3 Failure Follow-up (Documentation)
- [x] 5.1 openspec/changes/fix-parallel-on-merged-hook/proposal.md を追加して変更内容を記録する
- [x] 5.2 openspec/changes/fix-parallel-on-merged-hook/specs/ に仕様差分を追加し、実装が検証できるようにする
- [x] 5.3 openspec/changes/fix-parallel-on-merged-hook/tasks.md に実装/検証タスクを記載して完了状況を追跡する

## Acceptance #4 Failure Follow-up
- [x] Git作業ツリーがdirtyのため、未追跡ファイルを解消する: openspec/changes/fix-parallel-on-merged-hook/proposal.md, openspec/changes/fix-parallel-on-merged-hook/tasks.md, openspec/changes/fix-parallel-on-merged-hook/specs/hooks/spec.md, openspec/changes/fix-parallel-on-merged-hook/design.md
