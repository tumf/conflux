---
change_type: implementation
priority: medium
dependencies: []
references: []
---

**Change Type**: implementation

# fix-force-kill-selected-state

## Problem / Context

TUI の Running モードで、実行中の change に `K` → `Y` で force-kill した後、`[x]`（選択マーク）が残ったままになる。

この状態で `spc` キーを押すと、`selected=true` かつ `display_status_cache="not queued"` のため `AddToQueue` が発行され、意図しない再実行がかかる。

**根本原因**: オーケストレータが force-kill 時に発行する `OrchestratorEvent::ChangeDequeued` が、TUI の `handle_orchestrator_event` で処理されず `_ => {}` にフォールスルーしている。一方、同じ目的のレガシーイベント `ChangeStopped` 用のハンドラ `handle_change_stopped`（`selected = false` を設定）は存在するが、`ChangeStopped` は `#[allow(dead_code)]` であり誰からも発行されていない。

## Proposed Solution

`src/tui/state/event_handlers/mod.rs` の `handle_orchestrator_event` に、`OrchestratorEvent::ChangeDequeued` のハンドラを1行追加し、`handle_change_stopped` に委譲する。

これにより force-kill 時に `change.selected = false` と `display_status_cache = "not queued"` が正しく設定される。

## Acceptance Criteria

- Running モードで実行中の change を `K` → `Y` で force-kill した後、当該 change の `[x]` マークが消えること
- force-kill 後に `spc` を押しても再実行（`AddToQueue`）がかからないこと
- ユーザが明示的に `spc` で選択した場合のみ再度キューに入ること

## Out of Scope

- `ChangeStopped` レガシーイベントの削除（別 change で対応）
- `apply_display_statuses_from_reducer` での `selected` 同期強化（本 fix で十分）
