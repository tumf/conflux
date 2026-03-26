# Change: reducer-owned orchestration state for TUI consistency

## Why

状態バグの根本原因は、change の実行状態が 1 つの正規モデルとして設計されていないことにある。

- TUI は `QueueStatus` を局所的に保持し、イベント受信やキー操作で直接更新する
- orchestrator / parallel 実行側は `pending_changes`, `archived_changes`, `in_flight`, `merge_deferred_changes` などの集合で別管理している
- 5秒ごとの refresh は workspace 状態を見て TUI の `queue_status` を外側から上書きする

この結果、queue 操作・依存ブロック・`MergeWait` / `ResolveWait`・resolve 中の待機・停止後の遅延イベントが、同じ状態空間に属するのに別々の経路で更新され、仕様の誤解と race condition を生んでいる。

前回 draft の flat な `ChangePhase` 1 本化だけでは、これらの別軸の状態を 1 enum に押し込めてしまい、遷移表が再び壊れやすくなる。

## What Changes

1. `OrchestratorState` に reducer-owned の runtime state を導入し、change ごとの状態を階層化して管理する
2. change 状態を 1 つの flat enum ではなく、`queue_intent` / `activity` / `wait_state` / `terminal_state` / `workspace_observation` の直交した要素に分離する
3. 状態変更を reducer API (`apply_command`, `apply_execution_event`, `apply_observation`) に集約し、TUI は shared state の表示用 status を読むだけにする
4. 5秒 refresh は state を直接上書きせず、workspace observation を reducer に入力して reconcile する経路に変更する
5. `ResolveWait` は reducer-owned の待ち行列状態として扱い、workspace からの再構築対象にしない。workspace 由来の復元は `MergeWait` までに限定する

## Acceptance Criteria

- queue / blocked / merge wait / resolve wait / resolving / archived / merged / error の表示状態が、単一の reducer-owned runtime state から導出される
- TUI の Space / `M` / stop 操作は shared state を直接書き換えず、command handler 経由で reducer に intent を渡す
- refresh による workspace 観測は active state を上書きせず、定義された reconcile ルールに従ってのみ wait 状態を補正する
- `WorkspaceState::Archived` からの復元は `MergeWait` までとし、`ResolveWait` を workspace 観測だけで再生成しない
- duplicate / late / stale event は no-op または定義済みの優先順位で処理され、状態を退行させない

## Out of Scope

- Web API の payload shape 変更
- remote protocol の全面変更
- `pending_changes` / `archived_changes` など既存集計フィールドの全面削除
- reducer state の永続化

## Impact

- Affected specs:
  - new capability: `orchestration-state`
  - `tui-architecture`
  - `parallel-execution`
- Affected code:
  - `src/orchestration/state.rs`
  - `src/tui/state.rs`
  - `src/tui/command_handlers.rs`
  - `src/tui/runner.rs`
  - `src/tui/orchestrator.rs`
  - `src/parallel/`

This proposal supersedes the earlier flat-`ChangePhase` draft under the same change ID.
