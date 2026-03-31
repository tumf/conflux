---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/render.rs
---

# Fix: Running モードで error 状態の change に Space を押しても queued に遷移しない

**Change Type**: implementation

## Problem / Context

Running モードで error 状態の change に Space キーを押すと、`selected` フラグ（retry mark）のトグルのみが行われ、実際のステータスが `"error"` から `"queued"` に遷移しない。

根本原因は2つ:

1. `handle_toggle_running_mode()` の `"error"` アーム（`src/tui/state.rs:2809`）が `ToggleActionResult::StateOnly` を返し、`AddToQueue` コマンドを発行しない
2. `update_change_status()` のガード（`src/tui/state.rs:63`）が `"error"` → `"queued"` 遷移をブロックする

対照的に、F5 キーの `retry_error_changes()` は reducer に直接 `AddToQueue` を送り、`display_status_cache` を `"queued"` にセットしているため正しく動作する。

## Proposed Solution

1. `handle_toggle_running_mode()` の `"error"` ケースで、`selected = true` にする場合は `TuiCommand::AddToQueue` を発行する（`"not queued"` ケースと同じパターン）
2. `update_change_status()` の L63 ガードから `"error"` を除外し、`"error"` → `"queued"` 遷移を許可する
3. `selected = false`（retry mark 解除）の場合は `RemoveFromQueue` を発行する

## Acceptance Criteria

- Running モードで error 状態の change に Space を押すと `"queued"` に遷移する
- 再度 Space を押すと `"not queued"` に戻る（retry mark 解除）
- F5 による既存のリトライ機能に影響しない
- Stopped モードでの error change の Space 操作（mark のみ）は変更なし
- `"archived"` / `"merged"` → `"queued"` 遷移は引き続きブロックされる

## Out of Scope

- Stopped モードでの error change 操作の変更
- Error モード（F5 リトライ）のフローの変更
