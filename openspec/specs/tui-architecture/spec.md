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

TUI モジュールは公開エクスポートを維持しなければならない（SHALL）。

ただし、本プロジェクト内の機能追加に伴い、`OrchestratorEvent` および `TuiCommand` への **バリアント追加**は許容される（MAY）。

既存バリアントの意味・フィールド・名称の互換性は維持されなければならない（MUST）。

#### Scenario: 既存バリアントを壊さずに追加できる
- **GIVEN** external code imports from the tui module
- **WHEN** new variants are added to `OrchestratorEvent` or `TuiCommand`
- **THEN** existing variants remain available and unchanged
- **AND** the module continues to compile and run within this repository

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

TUI は実行イベントを受信して内部状態を更新しなければならない（SHALL）。

**変更内容**: `MergeCompleted` イベントを処理して、並列モードでマージが完了した変更のステータスを `Archived` に更新する。

#### Scenario: Parallel mode での MergeCompleted イベント受信

- **GIVEN** TUIが並列モードで実行中
- **WHEN** `ExecutionEvent::MergeCompleted { change_id, revision }` イベントを受信する
- **THEN** TUIは `change_id` に該当する変更のステータスを `Archived` に設定する
- **AND** 変更の `elapsed_time` を記録する（`started_at` から経過時間を計算）
- **AND** ログに "Merge completed for '{change_id}'" が追加される

#### Scenario: Archived ステータスは completed として表示される

- **GIVEN** 変更のステータスが `Archived` である
- **WHEN** TUIが変更リストをレンダリングする
- **THEN** 変更は緑色の "completed" (または "archived") として表示される
- **AND** ステータスが `UNCOMMITTED` や `NotQueued` として表示されることはない

