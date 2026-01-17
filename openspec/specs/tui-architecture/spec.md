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

**変更内容**:
- resolve待ちの変更は `NotQueued` ではなく、待機状態として視覚的に識別できる状態で表示する
- resolve待ち状態はユーザーの明示操作がない限り、auto-refresh やリスト更新で消失しない

#### Scenario: resolve待ち状態の表示を維持する
- **GIVEN** 変更が merge 待機状態として記録されている
- **WHEN** TUI が変更リストを再描画する
- **THEN** 変更のステータスは resolve待ちとして表示される
- **AND** `NotQueued` として表示されない

#### Scenario: resolve待ち状態は自動更新で保持される
- **GIVEN** 変更が resolve待ち状態である
- **WHEN** TUI が変更一覧を更新する
- **THEN** 変更の状態は resolve待ちのまま保持される
- **AND** ユーザー操作がない限りキューから外れた表示にならない

### Requirement: Log Entry Structure and Display
TUIのログエントリーは、タイムスタンプ、メッセージ、色に加えて、オプションのコンテキスト情報（change ID、オペレーション、イテレーション番号）を含まなければならない（SHALL）。
ログヘッダーは、利用可能なコンテキスト情報に基づいて段階的に表示される。

**構造体定義**:
```rust
pub struct LogEntry {
    pub timestamp: String,      // タイムスタンプ（HH:MM:SS形式）
    pub message: String,        // ログメッセージ
    pub color: Color,           // ログレベル色
    pub change_id: Option<String>,    // 変更ID
    pub operation: Option<String>,    // オペレーション ("apply", "archive", "resolve", "analysis", "ensure_archive_commit")
    pub iteration: Option<u32>,       // イテレーション番号（apply/archive/resolve/analysis）
}
```

**ビルダーメソッド**:
- `with_change_id(change_id: impl Into<String>)` - 変更IDを設定
- `with_operation(operation: impl Into<String>)` - オペレーションを設定
- `with_iteration(iteration: u32)` - イテレーション番号を設定

#### Scenario: ログヘッダーにオペレーションとイテレーションが表示される
- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="archive"`, `iteration=2` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:archive:2]` と表示される
- **AND** ヘッダーの後にメッセージが続く

#### Scenario: ensure_archive_commitのログが区別される
- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="ensure_archive_commit"`, `iteration=1` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:ensure_archive_commit:1]` と表示される
- **AND** archiveログと区別できる

#### Scenario: analysisログがイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id=None`, `operation="analysis"`, `iteration=3` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[analysis:3]` と表示される
- **AND** 解析の再実行が区別できる

#### Scenario: resolveログがイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id=None`, `operation="resolve"`, `iteration=2` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[resolve:2]` と表示される
- **AND** 解決の再実行が区別できる

#### Scenario: ログヘッダーに変更IDのみが表示される（後方互換性）
- **GIVEN** ログエントリーが `change_id="test-change"`, `operation=None`, `iteration=None` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change]` と表示される
- **AND** 既存の動作が維持される
