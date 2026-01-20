# Change: Refactor orchestrator state to unified shared model

## Why
オーケストレーションの状態管理がTUI/Webで重複し、`OrchestratorState`の定義が分散しているため理解と保守が難しい。単一の共有ステートに集約して、状態の一貫性と責務分離を明確にする。

## What Changes
- `orchestration/state.rs` を実際の共有ステートとして有効化し、TUI/Webの状態更新をこのモデルに統合する。
- Web用の状態DTOをリネームし、`OrchestratorState`の名称衝突を解消する。
- 共有ステートの更新をExecutionEvent駆動に統一し、TUI/Webの状態反映を参照ベースに揃える。

## Impact
- Affected specs: `tui-architecture`, `web-monitoring`
- Affected code: `src/orchestration/state.rs`, `src/web/state.rs`, `src/tui/state/*`, `src/events.rs`
