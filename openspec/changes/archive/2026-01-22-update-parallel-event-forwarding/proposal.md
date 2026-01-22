# Change: ParallelモードのTUIイベント転送順序を安定化

## Why
Parallel実行中にacceptanceが長時間走ると、TUIがApplying 100%で停止してAcceptingへ遷移しない。shared_stateの書き込みロック待ちでイベント転送がブロックされ、UI更新が遅延するため。

## What Changes
- parallel event転送で、TUIへの送信をshared_state更新より先に行う
- acceptance実行中でもTUIがAcceptingを表示できるようにする
- 既存のイベント内容や順序は維持し、送信の優先度のみ調整する

## Impact
- Affected specs: parallel-execution
- Affected code: src/tui/orchestrator.rs (run_orchestrator_parallelのforward_handle)
