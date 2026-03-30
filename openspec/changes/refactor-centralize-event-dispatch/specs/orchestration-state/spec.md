## MODIFIED Requirements

### Requirement: イベントディスパッチの一方向フロー
`ExecutionEvent` のステート遷移処理は `OrchestratorState::apply_execution_event()` でのみ行われなければならない（MUST）。

TUI (`AppState::handle_event`) および Web (`WebState::apply_execution_event`) は、ステータスフィールドを自前で書き換えるのではなく、Reducer の状態変更を読み取って表示に反映しなければならない（MUST）。

TUI/Web のイベントハンドラには、ログ追加・UI モード変更・elapsed_time 計測など、フロントエンド固有の処理のみを残すことができる（MAY）。

#### Scenario: Web がステータスを自前で書き換えない
- **WHEN** `ExecutionEvent::ProcessingStarted` が発行される
- **THEN** `OrchestratorState::apply_execution_event()` が `ChangeRuntimeState` を更新する
- **AND** `WebState` は Reducer の `display_status()` を読み取って `ChangeStatus.queue_status` を導出する
- **AND** `WebState` が `queue_status = Some("applying".to_string())` のような直接代入を行わない

#### Scenario: TUI がステータスを Reducer から反映する
- **WHEN** `OrchestratorEvent::ChangeArchived` が TUI に到達する
- **THEN** TUI は Reducer の `display_status()` を参照して表示ステータスを更新する
- **AND** TUI の `handle_event()` 内で `queue_status = QueueStatus::Archived` のような直接代入を行わない

#### Scenario: イベント送信元が3重送信しない
- **WHEN** オーケストレーションループがイベントを発行する
- **THEN** Reducer（`OrchestratorState`）にのみイベントが送信される
- **AND** Reducer の状態変更後にフロントエンド（TUI/Web）へ変更通知が送られる
- **AND** フロントエンドは通知を受けて Reducer のスナップショットを読み取り、表示を更新する
