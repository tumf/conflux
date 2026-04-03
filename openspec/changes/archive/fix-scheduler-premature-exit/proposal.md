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

通常の cflx は、ユーザが停止するまでイベントを受け続けるループ型アプリとして振る舞うべきである。一方で `run` サブコマンドだけは、与えられた job を処理し終えたら終了する事前 job 型の有限実行である。

現在の parallel scheduler は `queued` / `in_flight` / resolve wait などが一時的に空になると完了扱いで終了できる。これにより、通常のループ型 cflx 実行でも、ユーザが停止していないのに実行ループが終わり、その後に dynamic queue へ追加された change を拾えなくなる。

## Proposed Solution

- cflx の通常実行（TUI/server を含むループ型フロントエンド）では、parallel execution scheduler をユーザが停止するまで終了しない待機型ループに変更する
- `queued` と `in_flight` が一時的に空でも通常の cflx 実行ループは終了せず、dynamic queue 通知を待機する
- queued 追加後は既存の queue notification / debounce / re-analysis 経路で analyze を再開する
- `run` サブコマンドだけは例外として従来どおり有限実行とし、やることがなくなったら終了する
- 完了イベントと終了条件は「通常のループ型 cflx 実行」と「`run` の事前 job 型実行」で分離する

## Acceptance Criteria

- 通常の cflx 実行では `queued` と `in_flight` が一時的に空になっても parallel scheduler は終了しない
- ユーザが停止していない状態で change を queued に戻したとき、既存の通常実行ループが analyze を再開する
- `error -> queued` と通常の queue 追加の両方で同じ再分析経路を通る
- `run` サブコマンドだけは queued と実行中 work がなくなったら終了する
- 通常の cflx 実行の常駐化によって `run` サブコマンドの終了条件は変わらない

## Out of Scope

- serial モードのループ寿命変更
- 新しい UI モードやキー操作の追加
- queue 表示ラベルの見た目変更
