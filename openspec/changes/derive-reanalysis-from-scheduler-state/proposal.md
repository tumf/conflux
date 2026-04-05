---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/dynamic_queue.rs
---

# Reanalysis 判定をイベントフラグではなく scheduler 状態から導出する

**Change Type**: implementation

## Problem / Context

現在 scheduler は `needs_reanalysis: bool` フラグをイベント駆動で立て/落としている。
このフラグ管理には以下の構造的欠陥がある:

1. **立て忘れ**: QueueNotification の select 分岐で `needs_reanalysis = true` をセットしていないため、queue 通知を受けても reanalysis が起きない
2. **落とし過ぎ**: `perform_reanalysis_and_dispatch()` 末尾で無条件に `needs_reanalysis = false` にするため、dispatch できなかった場合に再試行意図が消える
3. **二重管理**: `needs_reanalysis` フラグは scheduler 状態 (queued, in_flight, available_slots, debounce) から常に導出可能であり、独立したフラグとして持つ必要がない

結果として、resolving 中にスロット空きがあるのに queued change が applying にならないケースが発生する。

## Proposed Solution

`needs_reanalysis` フラグを廃止し、毎ループ先頭で scheduler 状態から「reanalysis すべきか」を導出する。

判定ロジック:

```
should_reanalyze =
  !queued.is_empty()
  && available_slots > 0
  && debounce_elapsed()
```

これにより:
- イベントの種類に関係なく、条件を満たせば reanalysis が走る
- dispatch できなくても次ループで再評価される
- フラグの立て忘れ/落とし過ぎが構造的に起きない

## Acceptance Criteria

- `needs_reanalysis` フラグが廃止されている、または状態導出のキャッシュとしてのみ存在する
- 毎ループ先頭で queued + available_slots + debounce を評価して reanalysis 判定が行われる
- resolving 中にスロット空きがあり queued change がある場合、debounce 経過後に reanalysis → dispatch される
- 既存の SlotRecovery / ResolveCompletion による debounce bypass は引き続き動作する
- 既存テストが全て pass する

## Out of Scope

- debounce 時間やポリシーの変更
- ReanalysisReason enum の廃止（ログ用途として残す）
- Reducer 側の状態遷移修正
