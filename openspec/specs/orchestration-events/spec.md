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

### Requirement: Execution events identify their target change explicitly

Execution events that mutate per-change runtime activity SHALL identify the target change explicitly in the event payload. Events used to synchronize workspace activity from parallel execution MUST be applicable without consulting unrelated orchestrator-global cursors such as `current_change_id`.

#### Scenario: Workspace status update names its target change

- **GIVEN** a parallel workspace transitions into `Rejecting`
- **WHEN** the runtime emits a workspace status synchronization event
- **THEN** the event payload includes the target change identifier
- **AND** downstream reducers can update the matching runtime entry directly

### Requirement: Archive retry and resume events identify retry cause explicitly

Execution events used to synchronize archive retry or archive resume activity SHALL identify the target change and the archive retry/resume cause explicitly.

#### Scenario: archive retry event includes target change and reason

- **GIVEN** a parallel workspace for change `delta` schedules another archive attempt
- **WHEN** the runtime emits the archive retry synchronization event
- **THEN** the event payload includes the target change identifier
- **AND** the payload includes the archive primary reason and summary
- **AND** downstream reducers or UI layers can render why archive is looping without consulting unrelated global cursors or parsing free-form log text only
