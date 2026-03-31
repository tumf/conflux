---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
---

# Change: パラレルモードで resolve 中に queued change の dispatch がブロックされる問題を修正

**Change Type**: implementation

## Why

パラレルモードのスケジューラループ (`src/parallel/orchestration.rs`) で、`tokio::select!` の `join_set.join_next()` arm 内から `handle_workspace_completion` を await している。この関数は `handle_merge_and_cleanup` → `attempt_merge` → `merge_and_resolve` を呼び出し、コンフリクト解決（AI エージェント呼び出し）が必要な場合に長時間ブロックする。

`select!` の arm がブロックされている間、スケジューラループ全体が停止するため：
- queued な change の re-analysis & dispatch が行われない
- 他の in-flight タスクの完了通知も処理されない
- dynamic queue の新規追加もチェックされない

実行スロットが十分残っていても（例: 3 スロット中 resolve 1 つ）、ループ自体が止まるため queued change が dispatch されない。

## What Changes

- `handle_workspace_completion` 内の merge + resolve 処理をバックグラウンドタスクに分離し、`select!` arm を即座に完了させる
- merge/resolve の結果は既存の `join_set` または別の通知チャネル経由でスケジューラループに返す
- スケジューラループは merge/resolve 中も回り続け、queued change の dispatch を継続する

## Impact

- Affected specs: parallel-execution, parallel-merge
- Affected code: `src/parallel/orchestration.rs`, `src/parallel/queue_state.rs`, `src/parallel/merge.rs`

## Acceptance Criteria

- resolve（コンフリクト解決）中でも、残りスロットに応じて queued change が dispatch される
- merge/resolve の結果（成功・Deferred・失敗）が正しくスケジューラに伝達される
- `auto_resolve_count` の RAII ガード（`AutoResolveGuard`）がバックグラウンドタスク内でも正しく動作する
- `retry_deferred_merges` が merge 成功後に正しくトリガーされる
- 既存テスト（`tests/` 配下の parallel 関連）がすべて通過する

## Out of Scope

- シリアルモードの変更
- TUI の `is_resolving` フラグによる F5 ブロック（別問題）
- `calculate_available_slots` の resolve スロット消費ロジック変更（現行のスロット計算は正しい。問題はスケジューラループのブロッキング）
