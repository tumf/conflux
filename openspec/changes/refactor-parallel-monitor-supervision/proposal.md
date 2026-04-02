---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/merge_stall_monitor.rs
  - src/parallel/mod.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-events/spec.md
---

# Change: Refactor parallel monitor supervision

**Change Type**: implementation

## Problem/Context

`cflx queue` の実行経路では、並列実行のランニングループと進捗監視が同じ停止経路に結合されている。現在の `src/parallel/orchestration.rs` では `MergeStallMonitor` が並列実行開始時に共有 `cancel_token` を受け取り、`src/merge_stall_monitor.rs` は stall 検出時にその token を直接 cancel する。

この構造により、進捗監視の判断や不具合が queue の実行可否を左右してしまう。観測系と制御系の責務が混在しているため、監視ロジックの変更が並列実行の可用性を壊す設計になっている。

## Proposed Solution

- 並列実行のランニングループと進捗監視を分離し、監視は read-only な観測者として扱う
- `MergeStallMonitor` は `CancellationToken` を直接受け取らず、stall 検出イベントを supervisor/policy 層へ報告する
- 並列オーケストレーションは監視イベントを受けて warning や状態更新を行うが、デフォルトでは queue 実行を直接停止しない
- 停止判断が必要な場合は、監視器ではなく supervisor/policy 層が明示的に決定する構造へ変更する
- TUI/CLI の表示では、stall 検出を「実行失敗」ではなく独立した観測上の警告として扱う

## Acceptance Criteria

- `cflx queue` / 並列実行開始時に、進捗監視モジュール単体の判断で queue 実行全体が即キャンセルされない
- `MergeStallMonitor` は共有 `CancellationToken` を直接 cancel しない
- stall 検出は並列実行ループに対してイベントまたは状態通知として伝達される
- 監視イベントは warning / observability 用に記録され、通常の完了・停止・失敗イベントと混同されない
- 並列実行の停止理由は execution control 側が一元管理し、monitor 起因の停止は明示的な policy がある場合に限られる

## Out of Scope

- merge stall 検出アルゴリズム自体の閾値調整
- stall 発生時の自動復旧戦略の導入
- 並列実行以外の serial 実行系の全面的な supervisor 再設計
