# tui-architecture Specification

## Purpose
Defines the TUI module structure and architectural patterns.
## Requirements
### Requirement: TUI Module Structure

The TUI module SHALL be organized as a directory-based module tree under `src/tui/` with focused submodules.

#### Scenario: Module directory exists

- **WHEN** the project is compiled
- **THEN** `src/tui/mod.rs` exists as the module entry point
- **AND** submodules are organized in `src/tui/*.rs` files

#### Scenario: Each submodule has single responsibility

- **GIVEN** the TUI module structure
- **THEN** `types.rs` contains only enum and type definitions
- **AND** `state.rs` contains only state management logic
- **AND** `events.rs` contains only event and command definitions
- **AND** `render.rs` contains only rendering functions
- **AND** `orchestrator.rs` contains only orchestration logic
- **AND** `runner.rs` contains only the main TUI loop
- **AND** `queue.rs` contains only DynamicQueue implementation
- **AND** `utils.rs` contains only utility functions

### Requirement: Public API Stability

The TUI module SHALL maintain backward-compatible public exports.

#### Scenario: run_tui function exported

- **GIVEN** external code imports from the tui module
- **WHEN** `use crate::tui::run_tui` is called
- **THEN** the function is accessible
- **AND** the function signature is unchanged

#### Scenario: DynamicQueue type exported

- **GIVEN** external code imports from the tui module
- **WHEN** `use crate::tui::DynamicQueue` is called
- **THEN** the type is accessible
- **AND** all public methods are unchanged

#### Scenario: Event types exported

- **GIVEN** external code imports from the tui module
- **WHEN** `use crate::tui::{OrchestratorEvent, TuiCommand}` is called
- **THEN** the types are accessible
- **AND** all variants are unchanged

### Requirement: No Behavioral Changes

The TUI module refactoring SHALL NOT change any runtime behavior.

#### Scenario: All existing tests pass

- **WHEN** `cargo test` is run after refactoring
- **THEN** all tests that passed before refactoring still pass
- **AND** no new test failures are introduced

#### Scenario: TUI functionality unchanged

- **GIVEN** the TUI is started with `cargo run -- tui`
- **WHEN** user interacts with the TUI
- **THEN** all keyboard shortcuts work as before
- **AND** all display elements render identically
- **AND** all state transitions behave identically

### Requirement: Dynamic Queue Management
システムは、実行中にchangeを動的にキューへ追加・削除できる機能を提供しなければならない（SHALL）。

DynamicQueueは以下の操作をサポートすること：
- `push(id)`: change IDをキューに追加（重複チェック付き）
- `pop()`: キューから次のchange IDを取り出し
- `remove(id)`: 指定されたchange IDをキューから削除（新規追加）

#### Scenario: 実行中にキューに追加
- **WHEN** ユーザーがRunningモード中にSpaceキーでchangeを選択
- **THEN** DynamicQueueにchange IDが追加され、次回の処理で実行される

#### Scenario: 実行中にキューから削除
- **WHEN** ユーザーがRunningモード中に [x] のchangeをSpaceキーまたは@キーで [@] に変更
- **THEN** DynamicQueueから該当change IDが削除され、実行されない

#### Scenario: 重複追加の防止
- **WHEN** 既にキューに存在するchange IDを再度追加しようとする
- **THEN** 追加は拒否され、キューの状態は変わらない

#### Scenario: 存在しないIDの削除
- **WHEN** キューに存在しないchange IDを削除しようとする
- **THEN** エラーは発生せず、キューの状態は変わらない

### Requirement: Queue State Synchronization
システムは、UI上のキュー状態とDynamicQueueの状態を常に同期させなければならない（SHALL）。

#### Scenario: Unapproveによるキューからの削除
- **WHEN** ユーザーが@キーでqueuedのchangeをunapprove
- **THEN** `QueueStatus::NotQueued` に変更され、DynamicQueueからも削除される

#### Scenario: Spaceキーによるキューからの削除
- **WHEN** ユーザーがRunningモード中にSpaceキーで [x] のchangeをdequeue
- **THEN** `QueueStatus::NotQueued` に変更され、DynamicQueueからも削除される

#### Scenario: 削除操作のログ記録
- **WHEN** DynamicQueueからchangeが削除される
- **THEN** ログに削除操作が記録される

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

