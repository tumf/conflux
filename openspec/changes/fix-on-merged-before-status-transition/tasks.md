## 1. Parallel モード: merge_results() の on_merged を MergeCompleted 前に移動
- [ ] 1.1 `src/parallel/merge.rs` の `merge_results()` で `MergeAttempt::Merged` 分岐内の `on_merged` フック実行を、`attempt_merge()` が返した直後かつ `CleanupStarted` イベント送信前の位置に維持しつつ、`attempt_merge()` 内部の `MergeCompleted` イベント送信を `on_merged` の後に移動する（検証: `attempt_merge()` 内で `MergeCompleted` 送信前に caller へ制御を戻すか、caller 側で `on_merged` → `MergeCompleted` の順になっていること）

## 2. attempt_merge() から MergeCompleted 送信を caller に移動
- [ ] 2.1 `src/parallel/merge.rs` の `attempt_merge()` → `merge_and_resolve()` → `merge_and_resolve_with()` 内部の `MergeCompleted` イベント送信を削除し、`MergeAttempt::Merged` を返すだけにする（検証: `attempt_merge()` 内に `ParallelEvent::MergeCompleted` 送信が存在しないこと）
- [ ] 2.2 `merge_results()` の `MergeAttempt::Merged` 分岐で `on_merged` フック実行後に `MergeCompleted` イベントを送信する（検証: `on_merged` → `MergeCompleted` の順序であること）
- [ ] 2.3 `resolve_merge_for_change()` の `MergeAttempt::Merged` 分岐で `on_merged` フック実行後に `MergeCompleted` イベントを送信する（検証: 同上）

## 3. Parallel モード: 遅延リトライの on_merged を MergeCompleted 前に移動
- [ ] 3.1 `src/parallel/queue_state.rs` の遅延リトライ成功分岐で `on_merged` フック実行後に `MergeCompleted` イベントを送信する（検証: `on_merged` → `MergeCompleted` の順序であること）

## 4. TUI 手動マージ: on_merged を BranchMergeCompleted 前に移動
- [ ] 4.1 `src/tui/command_handlers.rs` で `BranchMergeCompleted` イベント送信の前に `on_merged` フック実行を移動する（検証: `on_merged` → `BranchMergeCompleted` の順序であること）

## 5. テスト
- [ ] 5.1 既存テスト `test_on_merged_hook_execution` が引き続きパスすることを確認する（検証: `cargo test test_on_merged_hook_execution`）
- [ ] 5.2 全テストがパスすることを確認する（検証: `cargo test`）
- [ ] 5.3 `cargo clippy -- -D warnings` がパスすることを確認する
