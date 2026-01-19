# Change: Parallel re-analysis dispatch during active batches

## Why
TUI の並列実行でキューに追加された変更が、実行中のバッチ完了まで取り込まれず、空きスロットが発生しても再分析と起動が遅れる。仕様で求められる「実行中でもキュー変更を監視し、空きスロットで即時起動する」挙動を回復する。

## What Changes
- 実行中の apply/archive バッチ待機ループでキュー通知を監視し、空きスロットがあれば即時に新しい変更を起動する
- キュー変化時の再分析通知が実行中でも反映されることを保証する
- 追加された変更の取り込みが、バッチ完了ではなく空きスロット基準で行われるようにする

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/mod.rs, src/tui/queue.rs（通知連携）, src/parallel_run_service.rs
