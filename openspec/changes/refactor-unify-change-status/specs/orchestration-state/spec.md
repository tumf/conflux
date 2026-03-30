## MODIFIED Requirements

### Requirement: 統一ステータス管理
Change のステータスは `OrchestratorState` 内の `ChangeRuntimeState` が唯一の正規ソースとなる。TUI および Web フロントエンドはステータスを読み取り専用で参照し、自前でステータスを保持・書き換えてはならない（SHALL NOT）。

ステータスの遷移は `OrchestratorState::apply_execution_event()` または `OrchestratorState::apply_command()` 経由でのみ行わなければならない（MUST）。

TUI・Web が表示用ステータス文字列を取得する場合は `ChangeRuntimeState::display_status()` を使用しなければならない（MUST）。

#### Scenario: TUI が Reducer からステータスを読み取る
- **WHEN** TUI が Change のステータスを表示する
- **THEN** `ChangeRuntimeState::display_status()` の返す文字列が使用される
- **AND** TUI ローカルにステータスを保持する enum（旧 `QueueStatus`）は存在しない

#### Scenario: Web API が Reducer からステータスを導出する
- **WHEN** Web API が `ChangeStatus` を生成する
- **THEN** `queue_status` フィールドは `ChangeRuntimeState::display_status()` から導出される
- **AND** `apply_execution_event()` 内でステータス文字列を独自に設定するロジックは存在しない

#### Scenario: ステータス遷移は Reducer 経由のみ
- **WHEN** ユーザ操作やオーケストレーションイベントによりステータスが変化する
- **THEN** `OrchestratorState` 上の Reducer API（`apply_execution_event` または `apply_command`）を経由して変更が行われる
- **AND** TUI/Web のコードが `ChangeState` や `ChangeStatus` のステータスフィールドを直接書き換えることはない
