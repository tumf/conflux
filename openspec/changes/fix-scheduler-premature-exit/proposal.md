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

- TUI/server で使う parallel execution scheduler だけを、ユーザが停止するまで終了しない待機型ループに変更する
- `queued` と `in_flight` が一時的に空でも TUI/server の実行ループは終了せず、dynamic queue 通知を待機する
- queued 追加後は既存の queue notification / debounce / re-analysis 経路で analyze を再開する
- `run` サブコマンドの parallel execution は従来どおり有限実行とし、やることがなくなったら終了する
- 完了イベントの意味はフロントエンド種別ごとに分離し、TUI/server の待機継続と CLI `run` の終了を混同しない

## Acceptance Criteria

- TUI/server の Running 中に `queued` と `in_flight` が一時的に空になっても parallel scheduler は終了しない
- ユーザが停止していない状態で change を queued に戻したとき、既存の TUI/server 実行ループが analyze を再開する
- `error -> queued` と通常の queue 追加の両方で同じ再分析経路を通る
- `run` サブコマンドだけは queued と実行中 work がなくなったら終了する
- TUI/server 側の常駐化によって `run` サブコマンドの終了条件は変わらない

## Out of Scope

- serial モードのループ寿命変更
- 新しい UI モードやキー操作の追加
- queue 表示ラベルの見た目変更
