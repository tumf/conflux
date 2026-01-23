# tui-architecture Specification

## Purpose
Defines the TUI module structure and architectural patterns.
## Requirements
### Requirement: TUI Module Structure

The TUI module SHALL be organized as a directory-based module tree under `src/tui/` with focused submodules. The TUI state layer MUST consume a shared orchestration state model for change progress and execution metadata, while UI-only fields (cursor, view modes, selection state) remain in TUI-owned state.

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

#### Scenario: Change progress uses shared state
- **GIVEN** the TUI state layer builds the change list for rendering
- **WHEN** change progress and execution metadata are required
- **THEN** the data source is the shared orchestration state
- **AND** UI-specific fields remain in TUI-owned state

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
TUI は 5 秒ごとの自動更新で `MergeWait` を評価し、以下のいずれかを満たす場合は `Queued` に戻さなければならない（MUST）。

- 対応する worktree が存在しない
- 対応する worktree が存在し、worktree ブランチが base に ahead していない

自動解除された change では `MergeWait` ではないため、`M` による merge resolve の操作ヒントや実行を行ってはならない（MUST NOT）。

#### Scenario: worktree がない場合は MergeWait を解除する
- **GIVEN** change が `MergeWait` である
- **AND** 対応する worktree が存在しない
- **WHEN** 5秒ポーリングの自動更新が実行される
- **THEN** change のステータスは `Queued` に戻る

#### Scenario: ahead なしの worktree は MergeWait を解除する
- **GIVEN** change が `MergeWait` である
- **AND** 対応する worktree が存在する
- **AND** worktree ブランチが base に ahead していない
- **WHEN** 5秒ポーリングの自動更新が実行される
- **THEN** change のステータスは `Queued` に戻る

#### Scenario: MergeWait が解除された change では M を使えない
- **GIVEN** change が `MergeWait` から `Queued` に戻っている
- **WHEN** TUI のキー表示が描画される
- **THEN** `M` による merge resolve のヒントは表示されない

### Requirement: Log Entry Structure and Display

TUIのログエントリーは、タイムスタンプ、メッセージ、色に加えて、オプションのコンテキスト情報（change ID、オペレーション、イテレーション番号）を含まなければならない（SHALL）。
ログヘッダーは、利用可能なコンテキスト情報に基づいて段階的に表示される。

- archive のログ出力は常にイテレーション番号を含み、ログヘッダーは `[{change_id}:archive:{iteration}]` 形式で表示されなければならない（MUST）。
- change_id のない analysis ログ出力は常にイテレーション番号を含み、ログヘッダーは `[analysis:{iteration}]` 形式で表示されなければならない（MUST）。
- ログの自動スクロールが無効な場合、TUIはユーザーが見ているログ範囲を保持し、新しいログ追加やログバッファのトリムで表示中の行が移動してはならない（MUST NOT）。表示中の行がトリムされた場合は残存ログの最古行へクランプし、オートスクロールを再有効化してはならない（MUST NOT）。

#### Scenario: archiveログは常にイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id="test-change"`, `operation="archive"`, `iteration=2` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[test-change:archive:2]` と表示される
- **AND** リトライの順序が判別できる

#### Scenario: analysisログはイテレーション付きで表示される
- **GIVEN** ログエントリーが `change_id=None`, `operation="analysis"`, `iteration=3` で作成される
- **WHEN** TUIがログをレンダリングする
- **THEN** ログヘッダーは `[analysis:3]` と表示される
- **AND** 解析の再実行が区別できる

#### Scenario: オートスクロール無効時の表示固定
- **GIVEN** ユーザーがログをスクロールしてオートスクロールが無効になっている
- **WHEN** 新しいログが追加される（必要に応じて古いログがトリムされる）
- **THEN** 既存の表示範囲は同じログ行を指し続ける
- **AND** 表示範囲がトリムされた場合は最古の残存ログ行へクランプされる
- **AND** オートスクロールは自動で再有効化されない

### Requirement: すべての状態でtasks.mdの進捗を反映する
TUIは、archive/resolving中であってもtasks.mdから取得できる進捗を表示し続けなければならない（MUST）。tasks.mdの読み取りが失敗し0/0になる場合、直前の進捗を上書きしてはならない（MUST NOT）。
自動更新処理において、active locationから0/0が返った場合はアーカイブ先を試し、それでも0/0なら既存値を保持しなければならない（MUST）。

#### Scenario: Archive/Resolving中に0/0が返る
- **GIVEN** 変更がArchivingまたはResolving状態である
- **AND** 直前のprogressが0/0ではない
- **WHEN** 自動更新でtasks.mdの取得に失敗し0/0が返る
- **THEN** 進捗表示は直前の値を維持する

#### Scenario: アーカイブ移動直後の自動更新で進捗を保持する
- **GIVEN** 変更がArchiving状態であり、worktree上でtasks.mdがアーカイブ先へ移動されている
- **AND** 直前のprogressが0/0ではない
- **WHEN** 自動更新で `parse_change_with_worktree_fallback` が0/0を返す
- **THEN** `parse_archived_change_with_worktree_fallback` を試みる
- **AND** それでも0/0なら既存の進捗値を保持する
