# Change: QueueStatus の二重管理を解消し、Reducer を唯一の状態源とする

**Change Type**: hybrid

## Why
現在、Change のステータス（Applying/Archiving/Merged 等）が3箇所で独立管理されている:
1. `orchestration::state::ChangeRuntimeState` — Reducer が所有する正規の状態（`ActivityState`, `WaitState`, `TerminalState`）
2. `tui::types::QueueStatus` — TUI 専用の enum で、Reducer とほぼ同じ状態遷移をローカルに保持
3. `web::state::ChangeStatus.queue_status` — Web 側が `Option<String>` で自前管理

この結果、以下の問題が発生している:
- TUI `apply_remote_status()` や `apply_display_statuses_from_reducer()` で Reducer → TUI への「ダウングレード防止」ロジックが複雑化
- Web `apply_execution_event()` が 20 以上の match arm で `queue_status` を文字列で書き換え、Reducer のロジックと完全に重複
- TUI `resolve_merge()` が `ChangeState.queue_status` を直接変更した上で Reducer にも同期するダブルライト構造

## What Changes
- `tui::types::QueueStatus` enum を廃止し、TUI `ChangeState` は `ChangeRuntimeState::display_status()` を参照するだけにする
- `web::state::ChangeStatus.queue_status` を Reducer の `display_status()` から導出し、Web 側の独自イベントハンドリングによるステータス管理を削除
- TUI/Web のフロントエンドコードがステートを「書き込む」箇所を排除し、Reducer 経由でのみ状態遷移を行う（読み取り専用パターン）

## Impact
- 影響する spec: `orchestration-state`
- 影響するコード: `src/tui/types.rs`, `src/tui/state.rs`, `src/web/state.rs`, `src/orchestration/state.rs`
- 依存関係: なし（単独で着手可能）

## Non-Goals
- Reducer のステートマシンロジック自体の変更
- 新しい状態の追加やワークフローの変更
- TUI/Web の見た目・レンダリングの変更
