---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/tui/command_handlers.rs
  - openspec/specs/parallel-execution/spec.md
---

# Change: Keep parallel scheduler alive until user stop

**Change Type**: implementation

## Problem/Context

TUI/server の Running 中に、error から再実行マークして queued に戻した change があっても analyze が再開されないことがある。

現在の parallel scheduler は `queued` / `in_flight` / resolve wait などが一時的に空になると完了扱いで終了できる。これにより、ユーザが停止していないのに実行ループが終わり、その後に dynamic queue へ追加された change を拾えなくなる。

## Proposed Solution

- parallel execution scheduler を、ユーザが停止するまで終了しない待機型ループに変更する
- `queued` と `in_flight` が一時的に空でも `AllCompleted` で実行ループを閉じず、dynamic queue 通知を待機する
- queued 追加後は既存の queue notification / debounce / re-analysis 経路で analyze を再開する
- 完了イベントの意味を「現在キューが空である」通知に限定し、実行ループの終了と切り離す

## Acceptance Criteria

- Running 中に `queued` と `in_flight` が一時的に空になっても parallel scheduler は終了しない
- ユーザが停止していない状態で change を queued に戻したとき、既存の実行ループが analyze を再開する
- `error -> queued` と通常の queue 追加の両方で同じ再分析経路を通る
- 実行ループの終了はユーザ停止または明示的なセッション終了時のみ起こる

## Out of Scope

- serial モードのループ寿命変更
- 新しい UI モードやキー操作の追加
- queue 表示ラベルの見た目変更
