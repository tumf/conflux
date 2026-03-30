# Change: Orchestrator/TUI orchestrator 間のループ状態を OrchestratorState に一元化する

**Change Type**: hybrid

## Why
オーケストレーションのループ状態（`apply_counts`, `pending_changes`, `changes_processed`, `iteration`, `current_change_id` 等）が以下の3箇所で独立して管理されている:

1. **`src/orchestrator.rs` (`Orchestrator` struct)** — `apply_counts: HashMap`, `changes_processed: usize`, `iteration: u32`, `current_change_id: Option<String>` をフィールドとして直接保持
2. **`src/tui/orchestrator.rs` (`run_orchestrator` 関数)** — `apply_counts: HashMap`, `pending_changes: HashSet`, `changes_processed: usize`, `total_changes: usize` をローカル変数で保持
3. **`src/orchestration/state.rs` (`OrchestratorState`)** — 同じ情報をすべて持つ「正規の共有ステート」

`OrchestratorState` が単一ソースとして設計されているにもかかわらず、`Orchestrator` struct と `run_orchestrator` 関数がそれぞれ独自にカウンタやセットを保持し、イベント発行時に `shared_state.write().await.apply_execution_event()` で同期するパターンになっている。これにより:
- 状態のズレが発生しうるリスク
- 変更を加える際に3箇所を同時に修正する必要がある保守コスト
- `Orchestrator` struct のフィールド数が過大（20以上）

## What Changes
- `Orchestrator` struct から `apply_counts`, `changes_processed`, `iteration`, `current_change_id`, `completed_change_ids`, `stalled_change_ids`, `skipped_change_ids` を削除し、`OrchestratorState` から読み取るように変更
- `tui/orchestrator.rs` の `run_orchestrator` からローカルの `apply_counts`, `pending_changes`, `changes_processed`, `total_changes` を削除し、`shared_state` を参照するように変更
- `OrchestratorState` に不足しているフィールド（`stalled_change_ids`, `skipped_change_ids` 等）があれば追加

## Impact
- 影響する spec: `orchestration-state`
- 影響するコード: `src/orchestrator.rs`, `src/tui/orchestrator.rs`, `src/orchestration/state.rs`
- 依存関係: `refactor-unify-change-status` と並行着手可能（依存なし）

## Non-Goals
- ループの制御フロー自体の変更（serial/parallel の使い分けは維持）
- `SerialRunService` や `ParallelRunService` の内部ロジック変更
- TUI/Web のレンダリングや見た目の変更
