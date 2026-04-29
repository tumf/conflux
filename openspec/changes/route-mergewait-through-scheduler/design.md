# Design: merge_wait retry intent is scheduler-owned

## Premise / Context

- 現行実装では `TuiCommand::ResolveMerge` が `src/tui/command_handlers.rs` から `resolve_deferred_merge(...)` を直接起動している。
- 並列実行の queued change analysis / dispatch / dependency gating / slot accounting は `ParallelExecutor` scheduler loop が担っている。
- そのため `MergeWait` 解消だけが scheduler 外の特殊経路になり、retry intent・completion reason・debounce bypass・status reconciliation の責務が二重化している。

## Goal

`MergeWait` 解消を通常 scheduler に統合し、TUI は「実行」ではなく「retry intent の入力」だけを担う。

## Non-Goals

- resolve conflict 解法の変更
- 新しい merge strategy の導入
- scheduler を複数系統に分割すること

## Current Problem

現在の責務分担は次のように分かれている。

1. `M` 押下
   - TUI state/reducer を更新
   - 同時に command handler が direct resolve task を `tokio::spawn`
2. 通常 scheduler
   - queue / dependency / slot / dispatch を評価
3. direct resolve 完了
   - `notify_scheduler()` と event 送信で scheduler を後追い起床

この構造では、`MergeWait` retry が scheduler 自身の lifecycle ではなく「外部で実行された結果を scheduler が後から知る」モデルになる。結果として、queue 追加や free slot の存在が通常どおり analysis / dispatch に反映されないケースが出る。

## Target Design

### 1. Intent ownership

- `ResolveMerge` は reducer-owned retry intent を表す。
- `ResolveWait` は reducer-owned queued resolve intent の一種として扱う。
- TUI は intent をセットし、scheduler wakeup を送るだけにする。

### 2. Execution ownership

- merge / resolve retry の開始主体は `ParallelExecutor` scheduler loop のみとする。
- scheduler は reducer-observable state、available slots、current resolving activity を見て retry を dispatch する。
- direct `resolve_deferred_merge(...)` 実行経路は廃止する。

### 3. Completion semantics

- resolve retry の成功 / 失敗 / cancel / clear は通常 scheduler completion と同じ semantics で reducer に反映する。
- `queued resolve wait cleared` と `retry evaluation should run now` は queue-only notify ではなく scheduler completion reason として扱えるようにする。

### 4. Queue interaction

- `MergeWait` retry intent が存在しても、available slot がある別 change の analysis / dispatch は通常どおり継続する。
- `Resolving` が 1 slot しか消費していない場合、残り slot は queued change dispatch に使える。
- scheduler は `merge_wait retry pending` と `queued changes` を同一 loop で判断する。

## Expected Repository Impact

- `src/tui/command_handlers.rs`: direct resolve spawn の削除、intent-only 化
- `src/orchestration/state.rs`: retry intent / queued resolve wait ownershipの明確化
- `src/parallel/orchestration.rs`: retry intent を見る scheduler loop
- `src/parallel/queue_state.rs`: retry intent と re-analysis / dispatch 条件の統合
- `src/tui/state*`: UI status/log の整合
- `src/parallel/tests/*`, `src/tui/state/*tests*`: regression coverage

## Verification Strategy

- reducer tests: `ResolveMerge` が intent 更新のみを行うこと
- scheduler tests: retry intent を観測して通常 loop から retry が始まること
- regression tests: `Resolving` 1件 + 空き slot + queue 追加で別 change が dispatch されること
- end-to-end-ish integration: retry completion 後に queued resolve wait と queued changes が通常 completion semantics で再評価されること
