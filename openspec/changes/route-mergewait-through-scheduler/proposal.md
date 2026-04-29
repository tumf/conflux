---
change_type: implementation
priority: high
references:
  - src/tui/command_handlers.rs
  - src/tui/key_handlers.rs
  - src/tui/state.rs
  - src/tui/orchestrator.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/tui/queue.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/tui-resolve-queue/spec.md
---

# Change: merge_wait 解消を通常 scheduler 経路へ統合する

**Change Type**: implementation

## Premise / Context

- 現行 TUI では `M` キーが `TuiCommand::ResolveMerge` を発行し、`src/tui/command_handlers.rs` で `resolve_deferred_merge(...)` を直接 `tokio::spawn` している。
- 一方で並列実行の re-analysis / dispatch は `ParallelRunService` と `ParallelExecutor` の scheduler loop が担っている。
- セッション内調査では、manual resolve 完了時の wakeup は `dynamic_queue.notify_scheduler()` に留まり、通常 scheduler の `ResolveCompletion` と同等の completion semantics を経由していない。
- ユーザ要求は、`M` を merge/resolve の専用実行オペレーションにせず、通常の parallel scheduler に対する再試行トリガーとして扱うことにある。
- 追加で、manual resolve 1件だけでは全 slot が埋まるわけではなく、空き slot があるのに queued change の analysis / dispatch が通常どおり進まない点も解消対象である。

## Requested Artifact

- implementation proposal for routing `MergeWait` retry intent through the normal parallel scheduler
- no standalone manual-resolve execution lane outside the scheduler

## Problem / Context

`MergeWait` change に対して `M` を押したとき、TUI command handler が通常 scheduler を経由せずに直接 resolve / merge 実行を始めている。このため、`queued` change の再 analysis、`ResolveWait` / `MergeWait` の reducer-owned intent、completion reason、debounce bypass、dispatch eligibility が同一 state machine で扱われない。

この分岐により、`merge_wait` 解消が「通常の parallel orchestration を進めるトリガー」ではなく、「TUI 専用の直接実行オペレーション」になっている。結果として、空き slot が残っていても queued change の analysis / dispatch が通常 scheduler の completion semantics とずれ、`merge_wait` 解消中や完了直後に queue が止まって見える。

## Proposed Solution

`M` を scheduler 外の direct execution ではなく、shared reducer と parallel scheduler に対する **resolve / merge retry intent** として扱う。

- `TuiCommand::ResolveMerge` は reducer-owned intent を記録し、scheduler wakeup を行うだけにする。
- `resolve_deferred_merge(...)` や merge retry 実行の主体は、TUI command handler ではなく通常の parallel scheduler / executor に一本化する。
- `MergeWait` / `ResolveWait` / queued follow-up は reducer-observable state から scheduler が評価し、available slot・dependency state・resolve serialization を同じ loop で判断する。
- manual resolve 完了後の reducer 更新、queued resolve wait clearing、re-analysis trigger は通常の `ResolveCompletion` 相当の completion semantics に統一する。
- queue に新規 change が積まれていて空き slot がある場合、`merge_wait` retry intent の存在は analysis / dispatch の通常進行を妨げない。

## Acceptance Criteria

- `M` 押下は merge / resolve 実行を直接開始せず、shared reducer と scheduler が観測できる retry intent の記録として扱われる。
- `MergeWait` change の merge / resolve 実行は通常の parallel scheduler 経路だけが開始する。
- manual resolve 完了・失敗・キャンセル後の queued resolve intent clearing と再評価は、通常 scheduler の completion semantics と同等に扱われる。
- `Resolving` 中の change が 1 件だけで `max_parallelism > 1` の場合、別 change の queue 追加は通常どおり analysis / dispatch 候補として扱われる。
- `MergeWait` 解消のために scheduler 外の TUI direct execution lane を前提とする挙動やコードパスが残らない。
- user-visible status / logs / reducer state は、`M` が direct execution ではなく scheduler-owned retry intent であることと整合する。

## Explicit Completion Conditions

- OpenSpec delta が `ResolveMerge` intent の ownership、scheduler-owned execution、completion semantics を canonical behavior として定義している。
- `src/tui/command_handlers.rs` から direct `resolve_deferred_merge(...)` spawn を外す実装タスクが tasks に明記されている。
- `src/parallel/orchestration.rs` / `src/parallel/queue_state.rs` 側で reducer-observable retry intent を dispatch / re-analysis に接続するタスクが tasks に含まれている。
- `queue を積んだまま merge_wait 解消をトリガーしたとき、空き slot があれば通常 dispatch が進む` ことを確認する回帰テストが tasks に含まれている。
- `cflx openspec validate route-mergewait-through-scheduler --strict --evidence warn` が成功する。

## Out of Scope

- resolve アルゴリズム自体の改善
- merge conflict 自動解決方針の再設計
- dashboard / Web UI の新規 UX 改修
