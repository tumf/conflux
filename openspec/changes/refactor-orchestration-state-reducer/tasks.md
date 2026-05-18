## Implementation Tasks

- [x] 現在の reducer 挙動を characterization test で固定する（verification: unit - `src/orchestration/state.rs` または分割後テストで `ReducerCommand::AddToQueue`、`ResolveMerge`、`DequeueChange`、`RetryError` の `ReduceOutcome` と display status が既存通りであることを確認する）
- [x] 主要 `ExecutionEvent` の状態遷移を characterization test で固定する（verification: unit - `ChangeArchived`、`MergeDeferred`、`MergeCompleted`、`ResolveStarted`、`ResolveCompleted`、`ResolveFailed`、`RejectionReviewCompleted` の terminal/activity/wait/queue intent と wait queue membership を確認する）
- [x] wait queue 操作を重複なく追跡できるヘルパーまたはサブモジュールへ整理する（verification: unit - resolve/reject wait queue の重複防止、片側 queue への移動、clear 処理の既存テストが通る）
- [x] `apply_command` の分岐を責務別関数へ分割し、公開 API と戻り値を維持する（verification: unit - コマンド characterization test が分割前と同じ期待値で成功する）
- [x] `apply_execution_event` の大きな match を lifecycle 領域別ヘルパーへ分割し、状態遷移順序を維持する（verification: unit - 実行イベント characterization test と既存 reducer テストが成功する）
- [x] 憲法制約に反しないことを確認する（verification: manual - 新しい out-of-worktree durable workflow state やログ/UI 状態を workflow-control 入力として追加していないことを差分確認する）
- [x] 対象検証を実行する（verification: manual - `cargo test orchestration::state` または該当 reducer テスト、`cargo fmt --check` を実行し、1秒超の新規重いテストを追加していないことを確認する）

## Future Work

- reducer 分割後にさらに状態型のファイル分割を進める場合は、別提案で扱う。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate refactor-orchestration-state-reducer --archive-gate`
