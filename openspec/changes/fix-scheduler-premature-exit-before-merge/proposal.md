---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
---

# Change: Scheduler が merge 実行前に早期終了するバグを修正

**Change Type**: implementation

## Why

parallel モードで change が1つの場合、archive 完了後に merge/resolve が一度も実行されず MergeWait で停止する。`spawn_merge_task` は `tokio::spawn` で background task を起動するが、scheduler の break 条件がこのタスクを追跡しないため、`in_flight=0, queued=0` で即座に scheduler が終了し、merge channel が close される。

ログ証拠:
```
Task completed: change='...', in_flight=0, available_slots=3
Change '...' completed successfully
All changes completed (queued/in-flight/resolve_wait/manual_resolve empty), stopping
Workspace '...' not found after archive completion, skipping merge
Failed to send merge result to scheduler loop: channel closed
```

## What Changes

- scheduler の break 条件（`orchestration.rs` L164-177, L202-211）に **pending merge task 数** を追加し、spawn 済み merge task が完了するまで scheduler を終了させない
- `spawn_merge_task` で pending カウンタを increment、`handle_merge_result` で decrement

## Impact

- Affected specs: parallel-merge
- Affected code: `src/parallel/orchestration.rs`, `src/parallel/queue_state.rs`, `src/parallel/mod.rs`
