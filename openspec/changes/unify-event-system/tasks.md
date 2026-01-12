# Tasks: イベントシステムの統一

## 1. 統一イベント型の設計

- [x] 1.1 `src/events.rs` を作成
- [x] 1.2 `ExecutionEvent` enum を定義
  - ProcessingStarted, ProcessingCompleted, ProcessingError
  - ApplyStarted, ApplyCompleted, ApplyFailed, ApplyOutput
  - ArchiveStarted, ArchiveCompleted, ArchiveFailed, ArchiveOutput
  - ProgressUpdated
  - WorkspaceCreated, WorkspaceResumed (parallel mode 用)
  - MergeStarted, MergeCompleted, MergeConflict (parallel mode 用)
  - HookStarted, HookCompleted, HookFailed (hooks 用)
  - Log, Stopped, AllCompleted, Error, ChangesRefreshed
- [x] 1.3 `src/main.rs` に `mod events;` を追加

## 2. Parallel モジュールの移行

- [x] 2.1 `src/parallel/mod.rs` を `ExecutionEvent` を使用するよう変更
- [x] 2.2 `src/parallel/executor.rs` を `ExecutionEvent` を使用するよう変更
- [x] 2.3 `src/parallel/events.rs` を `ExecutionEvent` の re-export に変更

## 3. TUI モジュールの移行

- [x] 3.1 `src/tui/events.rs` の `OrchestratorEvent` を `ExecutionEvent` の re-export に変更
- [x] 3.2 `src/tui/runner.rs` を `ExecutionEvent` を使用するよう変更
- [x] 3.3 `src/tui/orchestrator.rs` を `ExecutionEvent` を使用するよう変更
- [x] 3.4 `src/tui/state/events.rs` を `ExecutionEvent` を使用するよう変更

## 4. ブリッジレイヤーの更新

- [x] 4.1 `src/tui/parallel_event_bridge.rs` を更新して新しいイベントバリアントに対応
- [x] 4.2 `src/tui/parallel_event_bridge.rs` を削除（統一イベント型により不要に）
- [x] 4.3 `src/tui/mod.rs` から `parallel_event_bridge` の参照を削除
- [x] 4.4 `src/tui/orchestrator.rs` のブリッジ使用箇所を更新（直接転送に変更）

## 5. 互換性エイリアスの追加

- [x] 5.1 `OrchestratorEvent` を `ExecutionEvent` のエイリアスとして維持
- [x] 5.2 `ParallelEvent` を `ExecutionEvent` のエイリアスとして維持

## 6. テストの更新

- [x] 6.1 イベント関連のユニットテストを新しい型に更新
- [x] 6.2 統合テストが引き続き動作することを確認

## 7. 検証

- [x] 7.1 `cargo build` が成功すること
- [x] 7.2 `cargo test` が成功すること
- [x] 7.3 `cargo clippy` が警告なしで通ること
