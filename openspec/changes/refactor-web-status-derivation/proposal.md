---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/web/state.rs
  - src/orchestration/state.rs
---

# Change: Web ステータスを Reducer 導出に統一し apply_execution_event 内の冗長なステータス書き換えを削除する

**Change Type**: implementation

## Why
`src/web/state.rs` の `apply_execution_event()` メソッドに 14 箇所の `change.queue_status = Some("...".to_string())` 書き換え（L462, L476, L485, L509, L515, L526, L533, L572, L582, L589, L599, L684, L690）がある。一方で `from_changes_with_shared_state()` (L134) は既に Reducer の `display_status()` から `queue_status` を導出している。

この二重管理により：
- 新しいイベント追加時に `apply_execution_event()` と Reducer の両方を修正する必要がある
- イベントごとの文字列がReducer と微妙にずれるリスク
- `src/web/state.rs` が 1755 行と肥大化している一因

## Proposed Solution
`apply_execution_event()` 内のステータス文字列書き換え match arm を削除し、`from_changes_with_shared_state()` による Reducer 導出に完全に統一する。ログ追加・progress更新・worktree更新など非ステータス処理はそのまま残す。

## Acceptance Criteria
- `apply_execution_event()` 内に `queue_status = Some(...)` の直接代入が存在しない（テスト用コード除く）
- Web API が返す `queue_status` の値が変更前と同一（Reducer の `display_status()` と一致）
- `cargo test` 全パス
- `cargo clippy -- -D warnings` クリア
- Web ダッシュボードの表示に変化なし

## Out of Scope
- TUI の `QueueStatus` enum 廃止（別 proposal で段階的に実施）
- Reducer のステートマシンロジック変更
- Web API のレスポンス形式変更
