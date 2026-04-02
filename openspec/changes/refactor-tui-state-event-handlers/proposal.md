---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/tui-state-management/spec.md
  - openspec/specs/code-maintenance/spec.md
  - src/tui/state.rs
---

# Change: tui/state.rs のイベントハンドラ群をサブモジュールに分離する

**Change Type**: implementation

## Problem / Context

`src/tui/state.rs` は現在 5,822 行あり、`AppState` の 54 個の `pub fn` が同一ファイルに集中している。特に `handle_*` 系イベントハンドラ（約 30 関数）は 1,500 行以上を占め、状態遷移ロジック（`toggle_selection`, `resolve_merge` 等）やガード関数群もファイル後半に散在している。

既存の code-maintenance 仕様 (TUI State Module Structure) では `state/events/` 配下にイベント処理を分離する構成を定義しているが、現在の `state.rs` にはまだ大量の `handle_*` メソッドが直接記述されたまま残っている。

## Proposed Solution

`src/tui/state.rs` 内の `handle_*` イベントハンドラメソッドを以下のサブモジュール構成へ抽出する。

- `state/event_handlers/mod.rs` — `handle_orchestrator_event` ディスパッチャ
- `state/event_handlers/processing.rs` — `handle_processing_started`, `handle_apply_started`, `handle_archive_started`, `handle_acceptance_started`, `handle_resolve_started`, `handle_analysis_started`
- `state/event_handlers/completion.rs` — `handle_processing_completed`, `handle_all_completed`, `handle_change_archived`, `handle_merge_completed`, `handle_resolve_completed`, `handle_acceptance_completed`, `handle_branch_merge_started/completed/failed`
- `state/event_handlers/errors.rs` — `handle_processing_error`, `handle_apply_failed`, `handle_archive_failed`, `handle_resolve_failed`, `handle_change_stop_failed`, `handle_error`
- `state/event_handlers/output.rs` — `handle_apply_output`, `handle_archive_output`, `handle_acceptance_output`, `handle_analysis_output`, `handle_resolve_output`
- `state/event_handlers/refresh.rs` — `handle_changes_refreshed`, `handle_worktrees_refreshed`

`AppState` 構造体は `state/mod.rs` に残し、メソッドを `impl AppState` ブロックとして各サブモジュールで実装する。

## Acceptance Criteria

- `cargo fmt --check && cargo clippy -- -D warnings && cargo test` がすべて成功する
- TUI のキーボード操作・表示・状態遷移に変更がない
- `AppState` の public API (メソッド名・シグネチャ) に変更がない
- `state.rs` のイベントハンドラ部分が `state/event_handlers/` に移動し、`state.rs` (または `state/mod.rs`) の行数が大幅に削減される

## Out of Scope

- ガード関数群 (`validate_*`, `handle_toggle_*`) のさらなる分離
- `ChangeState` のサブモジュール化
- イベントハンドラの内部ロジック変更
