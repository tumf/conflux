# Change: on_merged フックを merged ステータス遷移の直前に実行する

## Why
現在 Parallel モードでは `attempt_merge()` 内部で `MergeCompleted` イベントが送信され merged ステータスに遷移した後に caller が `on_merged` フックを実行している。このため、フック内で「まだ merged になっていない」前提の処理（例: 最終検証やデータ収集）を行えない。`on_merged` は merged ステータス切り替えの直前に実行されるべきである。

## What Changes
- Parallel モード: `on_merged` フック実行を `MergeCompleted` イベント送信の前に移動する
  - `merge_results()` (`src/parallel/merge.rs`)
  - `resolve_merge_for_change()` (`src/parallel/merge.rs`)
  - 遅延リトライ (`src/parallel/queue_state.rs`)
- TUI 手動マージ: `on_merged` フック実行を `BranchMergeCompleted` イベント送信の前に移動する (`src/tui/command_handlers.rs`)
- Serial モード: 変更なし（既に正しい順序）
- hooks 仕様: `on_merged` の実行タイミングを「マージ完了後」から「マージ成功後、merged ステータス遷移の直前」に明確化する

## Impact
- Affected specs: hooks
- Affected code: `src/parallel/merge.rs`, `src/parallel/queue_state.rs`, `src/tui/command_handlers.rs`
- **BREAKING**: `on_merged` フック内で merged ステータスが既に設定されていることに依存する処理がある場合は影響を受ける（そのようなユースケースは現状存在しない）
