## MODIFIED Requirements

### Requirement: Web ステータスは Reducer から導出される
Web API が返す `ChangeStatus.queue_status` は `OrchestratorState` 内の `ChangeRuntimeState::display_status()` から導出されなければならない（MUST）。

`WebState::apply_execution_event()` メソッドは `queue_status` フィールドを直接書き換えてはならない（SHALL NOT）。ステータスの遷移は `OrchestratorState::apply_execution_event()` で行い、Web 層は Reducer のスナップショットから `queue_status` を読み取る。

#### Scenario: apply_execution_event がステータスを直接設定しない
- **WHEN** `ExecutionEvent::ProcessingStarted` が `WebState::apply_execution_event()` に渡される
- **THEN** `ChangeStatus.queue_status` が `Some("applying".to_string())` のように直接代入されない
- **AND** `queue_status` は次回の `from_changes_with_shared_state()` 呼び出し時に Reducer の `display_status()` から導出される

#### Scenario: Web API レスポンスの queue_status が Reducer と一致する
- **WHEN** Web API が `/api/state` エンドポイントでステータスを返す
- **THEN** 各 Change の `queue_status` が `ChangeRuntimeState::display_status()` の返す文字列と一致する
