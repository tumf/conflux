# Change: merge_deferred_changes を ResolveWait と MergeWait に分離し scheduler loop の終了条件を修正する

**Change Type**: implementation

## Why

`merge_deferred_changes: HashSet<String>` が ResolveWait（自動リトライ待ち＝進行中）と MergeWait（ユーザー介入待ち＝中断）を区別していない。scheduler loop の break 条件は `queued.is_empty() && in_flight.is_empty()` のみで判定しており、ResolveWait の change が残っていても scheduler が終了してしまう。

その結果、resolve 進行中に新たな change を queue に追加しても analyze / dispatch されない。

ResolveWait は「進行中」であり scheduler loop は回り続けるべきだが、MergeWait は「ユーザー待ち」であり scheduler loop を止めてよい。この区別がないのが根本原因。

## What Changes

- `merge_deferred_changes: HashSet<String>` を `resolve_wait_changes: HashSet<String>` と `merge_wait_changes: HashSet<String>` に分離する
- MergeDeferred イベント受信時に `auto_resumable` フラグで振り分ける
- scheduler loop の break 条件に `resolve_wait_changes.is_empty()` と `manual_resolve_active() == 0` を追加する
- `retry_deferred_merges` の対象を `resolve_wait_changes` のみにする
- `handle_all_completed` の Resolving ワークアラウンド（`state.rs:1656-1668`）を除去する

## Impact

- Affected specs: `parallel-execution`, `tui-resolve`
- Affected code:
  - `src/parallel/mod.rs` — フィールド定義
  - `src/parallel/builder.rs` — 初期化
  - `src/parallel/merge.rs` — `handle_merge_and_cleanup`, `retry_deferred_merges`
  - `src/parallel/queue_state.rs` — `has_merge_deferred`, `retry_deferred_merges`
  - `src/parallel/orchestration.rs` — break 条件
  - `src/tui/state.rs` — `handle_all_completed` ワークアラウンド除去
  - `src/parallel/tests/executor.rs` — 既存テスト更新
