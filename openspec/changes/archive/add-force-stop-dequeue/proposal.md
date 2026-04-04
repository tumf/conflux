---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/server-api/spec.md
  - src/orchestration/state.rs
  - src/tui/command_handlers.rs
  - src/tui/state.rs
  - src/server/api.rs
---

# Change: 実行中 change を強制停止して not queued に戻す

**Change Type**: implementation

## Problem / Context

実行中の change に対して現在の停止操作を行うと、reducer は `TerminalState::Stopped` を設定し、表示状態は `stopped` になる。

この挙動では「いったん実行を中断してキューから外し、通常の `not queued` 状態へ戻したい」という運用ニーズを満たせない。特に TUI と server/dashboard の両方で、実行中 change を明示的に dequeue したい場面がある。

既存コードでは `StopChange` が terminal state を作るため、単なる queue 解除と意味論が異なる。今回必要なのは terminal 化ではなく、実行キャンセル完了後に reducer を `not queued` / `idle` / `terminal none` に戻す操作である。

## Proposed Solution

実行中 change に対する新しい「強制停止してキュー解除」操作を追加する。

- reducer には既存 `StopChange` とは別の dequeue 意図を表すコマンド/イベントを追加する
- ランナーは対象 change の実行をキャンセルし、停止確認後に reducer を `queue_intent = NotQueued`, `activity = Idle`, `wait_state = None`, `terminal = None` へ遷移させる
- TUI では active 状態の toggle/stop 操作をこの新しい意味論に合わせる
- server/dashboard では change 単位の stop-and-dequeue API を提供し、REST と WebSocket の状態反映を揃える
- archived / merged / rejected のような完了済み terminal change には適用しない

## Acceptance Criteria

- 実行中 change を強制停止した後、表示状態が `stopped` ではなく `not queued` になる
- 停止後の change は reducer 上で terminal state を持たない
- 停止後の change はユーザーが再キューするまで自動再開しない
- TUI と server/dashboard で同じ意味論の操作が利用できる
- stale event や refresh により `not queued` から active 状態へ不正に戻らない

## Out of Scope

- 実行途中 worktree を初期状態へ自動巻き戻しすること
- 途中で生成された WIP commit や作業痕跡の自動クリーンアップ
- archived / merged / rejected change を not queued へ戻すこと
