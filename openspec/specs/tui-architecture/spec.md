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
- `MergeCompleted` イベント処理時に、状態を `Archived` ではなく **`Merged`** に設定する
- `QueueStatus` enum に **`Merged`** 状態を追加し、並列モードの最終状態として使用する

#### Scenario: Parallel mode での MergeCompleted イベント受信時に Merged 状態に遷移

- **GIVEN** TUIが並列モードで実行中
- **WHEN** `ExecutionEvent::MergeCompleted { change_id, revision }` イベントを受信する
- **THEN** TUIは `change_id` に該当する変更のステータスを **`Merged`** に設定する
- **AND** 変更の `elapsed_time` を記録する（`started_at` から経過時間を計算）
- **AND** ログに "Merge completed for '{change_id}'" が追加される

#### Scenario: Merged ステータスは terminal state として扱われる

- **GIVEN** 変更のステータスが `Merged` である
- **WHEN** TUIが terminal state をチェックする
- **THEN** `Merged` は `Archived`, `Completed`, `Error` と同様に terminal state として扱われる
- **AND** Progress 更新は実行されない
- **AND** リスト更新時も保持される

#### Scenario: Merged ステータスは明確に表示される

- **GIVEN** 変更のステータスが `Merged` である
- **WHEN** TUIが変更リストをレンダリングする
- **THEN** ステータスが "merged" として表示される
- **AND** 色は `Color::LightBlue` で表示される（青系で "完了かつ統合済み" を表現）
- **AND** チェックボックスは `[x]` でグレーアウト表示される

#### Scenario: Serial モードでは Archived が最終状態として維持される

- **GIVEN** Serial モードで実行中
- **WHEN** 変更がアーカイブされる
- **THEN** 状態は `Archived` となる
- **AND** `Merged` 状態には遷移しない
- **AND** `Archived` が最終状態として扱われる

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
    pub operation: Option<String>,    // オペレーション ("apply", "archive", "resolve")
    pub iteration: Option<u32>,       // イテレーション番号（applyのみ）
}
```

**ビルダーメソッド**:
- `with_change_id(change_id: impl Into<String>)` - 変更IDを設定
- `with_operation(operation: impl Into<String>)` - オペレーションを設定
- `with_iteration(iteration: u32)` - イテレーション番号を設定

#### Scenario: ログヘッダーにオペレーションとイテレーションが表示される

- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="apply"`, `iteration=1` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:apply:1]` と表示される
- **AND** ヘッダーの後にメッセージが続く

#### Scenario: ログヘッダーにオペレーションのみが表示される

- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="archive"`, `iteration=None` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:archive]` と表示される
- **AND** イテレーション番号は表示されない

#### Scenario: ログヘッダーに変更IDのみが表示される（後方互換性）

- **GIVEN** ログエントリーが `change_id="test-change"`, `operation=None`, `iteration=None` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change]` と表示される（従来の形式）
- **AND** 既存の動作が維持される

#### Scenario: ログヘッダーが表示されない

- **GIVEN** ログエントリーが `change_id=None` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ヘッダーは表示されず、タイムスタンプとメッセージのみが表示される

#### Scenario: ビルダーメソッドでコンテキスト情報を設定できる

- **GIVEN** LogEntry::info("message") でログエントリーが作成される
- **WHEN** `.with_change_id("test")`, `.with_operation("apply")`, `.with_iteration(2)` を連鎖呼び出しする
- **THEN** ログエントリーの各フィールドが正しく設定される
- **AND** ヘッダーは `[test:apply:2]` と表示される

#### Scenario: 並列実行時のapplyログにイテレーション番号が含まれる

- **GIVEN** 並列実行モードでapply操作が実行される
- **WHEN** イテレーション1のapplyログが生成される
- **THEN** ログヘッダーは `[change_id:apply:1]` 形式で表示される
- **AND** 複数回のイテレーションが区別できる

#### Scenario: archive操作のログにオペレーションタイプが含まれる

- **GIVEN** 変更のarchive操作が実行される
- **WHEN** archiveログが生成される
- **THEN** ログヘッダーは `[change_id:archive]` 形式で表示される
- **AND** apply操作と区別できる

#### Scenario: resolve操作のログにオペレーションタイプが含まれる

- **GIVEN** コンフリクト解決操作が実行される
- **WHEN** resolveログが生成される
- **THEN** ログヘッダーは `[change_id:resolve]` 形式で表示される
- **AND** 他の操作と区別できる

#### Scenario: ログヘッダーの表示幅が適切に計算される

- **GIVEN** より長いログヘッダー形式 `[change_id:operation:iteration]` が使用される
- **WHEN** TUIがメッセージの利用可能幅を計算する
- **THEN** ヘッダー全体の長さが考慮される
- **AND** メッセージが適切に切り詰められる
- **AND** ターミナル幅を超えない

