---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/state.rs
  - src/openspec.rs
  - src/tui/state.rs
  - openspec/specs/orchestration-state/spec.md
---

# Change: REJECTED marker removal reactivates change

**Change Type**: implementation

## Premise / Context

- base branch 上の `openspec/changes/<change-id>/REJECTED.md` は durable rejection marker として扱われ、marker-bearing change は active listing から除外される。
- 現在の reducer は `ExecutionEvent::ChangeRejected` 後に `TerminalState::Rejected` を保持し、`AddToQueue` を拒否する。
- `ChangesRefreshed` は worktree/archive 観測は反映するが、base branch から `REJECTED.md` が削除された change の terminal rejected 解除を行わない。
- その結果、ユーザが base branch で `REJECTED.md` を削除しても UI / reducer 上では `act: rejected` のまま残り、`exp: not queued/queued` と整合しない。

## Problem / Context

現行仕様では、rejected change は base branch 上の `REJECTED.md` marker によって active list から除外される。一方で runtime reducer も `TerminalState::Rejected` を保持するため、base branch から marker を削除して change が再び active listing に戻ってきても、in-memory state は自動で復帰しない。

このため refresh 後も change が `rejected` と表示され続け、通常の `not queued` change として再キューできない。ユーザは rejected marker を削除して change を再開したいのに、runtime state がそれを妨げている。

## Proposed Solution

`ChangesRefreshed` を active change list の真実源として扱い、base branch から `REJECTED.md` が削除されて active listing に戻ってきた change は reducer 上でも rejected terminal state から復帰させる。

- refresh で active list に含まれる change について、過去の `TerminalState::Rejected` を維持しない
- reactivated change は `terminal = None`, `activity = Idle`, `wait_state = None`, `queue_intent = NotQueued` に戻す
- 復帰後は通常の change と同様に `AddToQueue` を受理できるようにする
- TUI / Web の表示は refresh 後に `rejected` ではなく `not queued` / `queued` へ収束させる
- marker が残っている change の exclusion semantics は維持する

## Acceptance Criteria

- base branch で `openspec/changes/<change-id>/REJECTED.md` が削除され、次回 refresh で change が active listing に戻った場合、runtime は `Rejected` terminal state を解除する
- reactivated change の refresh 後表示は `rejected` ではなく `not queued` になる
- reactivated change は `AddToQueue` により再キューできる
- marker が残っている change は引き続き active listing から除外される
- reducer / TUI / Web の表示は refresh 後に同じ再活性化結果へ収束する

## Out of Scope

- rejected marker を archive へ移す長期保管方針の再設計
- rejection flow 自体の commit 内容や reject review protocol の変更
- file watcher による即時反映; 本変更は refresh ベースの収束を対象とする
