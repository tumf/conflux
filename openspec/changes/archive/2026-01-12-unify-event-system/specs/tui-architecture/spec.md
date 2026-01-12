# tui-architecture spec delta

## ADDED Requirements

### Requirement: Event-Driven State Updates

TUI は統一された `ExecutionEvent` 型を使用して状態更新を受信しなければならない（SHALL）。

#### Scenario: Serial mode でのイベント受信

- **GIVEN** TUI が serial mode で実行されている
- **WHEN** orchestrator が `ExecutionEvent::ProcessingStarted` を発行する
- **THEN** TUI の状態が更新され、処理中の change が表示される

#### Scenario: Parallel mode でのイベント受信

- **GIVEN** TUI が parallel mode で実行されている
- **WHEN** executor が `ExecutionEvent::WorkspaceCreated` を発行する
- **THEN** TUI の状態が更新され、ワークスペース情報が表示される

#### Scenario: イベントブリッジの廃止

- **GIVEN** parallel mode でイベントが発行される
- **WHEN** TUI がイベントを受信する
- **THEN** ブリッジレイヤーを経由せずに直接 `ExecutionEvent` を処理する

## REMOVED Requirements

None. The TUI architecture requirements remain, but internal implementation is updated to use `ExecutionEvent`.
