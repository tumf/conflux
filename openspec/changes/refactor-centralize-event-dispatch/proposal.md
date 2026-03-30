# Change: ExecutionEvent のディスパッチを一元化し、TUI/Web の重複ハンドラを排除する

**Change Type**: hybrid

## Why
`ExecutionEvent` の処理が以下の3箇所で独立して行われている:

1. **`orchestration::state::OrchestratorState::apply_execution_event()`** — Reducer が正規の状態遷移を実行（2205行ファイル内）
2. **`web::state::WebState::apply_execution_event()`** — 同名メソッドが 30 以上の match arm で `ChangeStatus` の `queue_status`・`iteration_number`・`status` を独自に書き換え（1755行ファイル内）
3. **`tui::state::AppState::handle_event()`** — `OrchestratorEvent`（`ExecutionEvent` の TUI ラッパー）を 30 以上の match arm で処理し、`ChangeState.queue_status` を独自に遷移

呼び出し側（`tui/orchestrator.rs` L272-282 等）では:
```rust
let _ = tx.send(event.clone()).await;                      // → TUI に送信
shared_state.write().await.apply_execution_event(&event);  // → Reducer に送信
ws.apply_execution_event(&event).await;                    // → Web にも送信
```
と、同一イベントを3箇所に個別送信し、それぞれが独自にステータスを書き換えている。

この構造により:
- 新しいイベント種別追加時に3箇所を同時に修正する必要がある
- 各ハンドラの状態遷移ロジックが微妙に異なるリスク（実際に `MergeDeferred` の `auto_resumable` 判定で差異あり）
- コードの認知負荷が高い（Web state.rs だけで 1755 行）

## What Changes
- Reducer（`OrchestratorState::apply_execution_event`）を唯一のステートマシンとして確立し、TUI/Web は「Reducer のスナップショットを読み取って表示を更新する」パターンに統一
- `WebState::apply_execution_event()` からステータス遷移ロジックを削除し、Reducer の `display_status()` からの導出に統一
- `AppState::handle_event()` のステータス遷移部分を Reducer への委譲に置き換え（ログ追加・UI 固有操作のみ残す）
- イベントのディスパッチを「Reducer → フロントエンドに通知」の一方向フローに変更

## Impact
- 影響する spec: `orchestration-state`
- 影響するコード: `src/web/state.rs`, `src/tui/state.rs`, `src/tui/orchestrator.rs`, `src/orchestrator.rs`, `src/orchestration/state.rs`
- 依存関係: `refactor-unify-change-status` の完了後に着手するのが理想（ステータス一元化が前提となるため）

## Non-Goals
- イベント種別の追加・削除
- `ExecutionEvent` enum 自体の構造変更
- TUI のログ表示やキーハンドリングの変更
- Web API のレスポンス形式の変更
