
### Requirement: イベント送信はヘルパー関数経由で行う
オーケストレーションループからの `ExecutionEvent` 送信は `dispatch_event()` ヘルパー関数を経由しなければならない（MUST）。個別に `tx.send()`, `shared_state.write().await.apply_execution_event()`, `ws.apply_execution_event()` を直接呼び出すパターンを使ってはならない（SHALL NOT）。

#### Scenario: TUI オーケストレータがヘルパー経由でイベントを送信する
- **WHEN** `tui/orchestrator.rs` の `run_orchestrator` 内でイベントを送信する
- **THEN** `dispatch_event()` ヘルパーが呼ばれる
- **AND** ヘルパー内で Reducer (shared_state), TUI channel (tx), Web (web_state) の3箇所に送信される

#### Scenario: CLI オーケストレータも同じヘルパーを使う
- **WHEN** `orchestrator.rs` 内で Web 向けにイベントを送信する
- **THEN** 同様のヘルパー関数が使われる
- **AND** 直接 `apply_execution_event()` を呼び出さない
