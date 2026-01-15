## Context
Web 監視（web-monitoring）は `WebState` を中心に状態を保持し、WebSocket へ `state_update` をブロードキャストする。

一方、parallel 実行は Git worktree を利用し、進捗（`tasks.md` の完了数）は worktree 内で更新される。したがって、ベース作業ツリー側の `openspec/changes/**/tasks.md` を読み直しても、必ずしも progress が増えない。

また、parallel 実行は `ExecutionEvent`（旧 ParallelEvent）を発行しており、`ProgressUpdated` などのイベントから worktree 内で観測した進捗を取得できる。

## Goals
- parallel 実行中でも、Web ダッシュボードが正しい進捗とステータスを表示する。
- WebSocket 接続中はイベント駆動で更新し、ポーリング依存を避ける。

## Non-Goals
- Web UI を部分更新対応に作り替える。
- エラー状態の視覚表現（error badge 等）を追加する。

## Decision
### Decision 1: parallel の `ExecutionEvent` を Web に橋渡しする
- `ParallelRunService::run_parallel` のコールバックは `Fn(ExecutionEvent)` で `async` ではない。
- そのため、コールバック内では `mpsc` などでイベントを送るだけにし、別 `tokio::spawn` タスクで `WebState` を更新する。

### Decision 2: WebSocket の `state_update` は常に「全件スナップショット」を送る
現状の Web UI は `state_update` 受信時に `changes` 配列をそのまま描画し直すため、サーバが差分（1件のみ）を送ると一覧が欠落する。
そのため、イベント駆動更新でも `changes` は全件スナップショットに統一する。

## Event → WebState 反映ルール（最小）
- `ProcessingStarted(change_id)`: 当該 change の status を `in_progress` として扱う（表示上の遷移を保証）
- `ProgressUpdated { change_id, completed, total }`: 完了数と合計数を更新し、status を `pending/in_progress/complete` に再計算
- `ChangeArchived(change_id)`: 当該 change を `complete` として扱う（UI 集計互換）

## Risks / Trade-offs
- イベントのみで status を補完するため、ベース作業ツリーの `openspec list` と Web 表示の情報源が混在する。
  - ただし Web 監視は「観測可能性」の目的であり、parallel の真値はイベント側にある。

## Open Questions
- `ApplyFailed` / `ProcessingError` を Web UI にどう表現するか（今回は非ゴール）
