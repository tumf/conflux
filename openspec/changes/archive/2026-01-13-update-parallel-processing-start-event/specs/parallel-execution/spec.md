## MODIFIED Requirements
### Requirement: Parallel Execution Event Reporting

parallel 実行モジュールは、統一された `ExecutionEvent` 型を使用してイベントを発行しなければならない（SHALL）。

#### Scenario: Workspace 作成イベント

- **GIVEN** parallel executor が change 用のワークスペースを作成する
- **WHEN** ワークスペースの作成が完了する
- **THEN** `ExecutionEvent::WorkspaceCreated` が発行される
- **AND** イベントには change_id と workspace path が含まれる

#### Scenario: ProcessingStarted の早期発行

- **GIVEN** parallel executor が change のワークスペースを作成または再利用する
- **WHEN** change の処理準備が完了する
- **THEN** `ExecutionEvent::ProcessingStarted(change_id)` が発行される
- **AND** TUI は該当 change を processing 状態として表示する

#### Scenario: Apply 進捗イベント

- **GIVEN** parallel executor が change を処理している
- **WHEN** apply コマンドが完了し進捗が更新される
- **THEN** `ExecutionEvent::ProgressUpdated` が発行される
- **AND** イベントには completed と total タスク数が含まれる

#### Scenario: マージ完了イベント

- **GIVEN** parallel executor が複数の change をマージする
- **WHEN** マージが成功する
- **THEN** `ExecutionEvent::MergeCompleted` が発行される
- **AND** イベントにはマージされた change_ids とリビジョンが含まれる
