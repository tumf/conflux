# Change: Orchestrator/TUI orchestrator 間のループ状態を OrchestratorState に一元化する

**Change Type**: hybrid

## Why
オーケストレーションのループ状態は当初3箇所で独立管理されていたが、大部分は既に `OrchestratorState` (shared_state) への移行が完了している。ただし一部のフィールド（`max_iterations` 等）が `Orchestrator` struct に残留している可能性があり、完全な一元化を確認・完了する必要がある。

## What Changes
- `Orchestrator` struct に `OrchestratorState` と重複するフィールドが残っている場合、削除して `shared_state` 経由のアクセスに統一する
- `tui/orchestrator.rs` にローカル変数として状態を保持している箇所があれば同様に `shared_state` 経由に統一する
- 重複がなければ、確認結果を記録して本 proposal を archive する

## Impact
- 影響する spec: `orchestration-state`
- 影響するコード: `src/orchestrator.rs`, `src/tui/orchestrator.rs`, `src/orchestration/state.rs`
- 依存関係: なし

## Non-Goals
- ループの制御フロー自体の変更
- `SerialRunService` や `ParallelRunService` の内部ロジック変更
