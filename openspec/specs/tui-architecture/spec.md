# tui-architecture Specification

## Purpose
Defines the TUI module structure and architectural patterns.
## Requirements
### Requirement: TUI Module Structure

The TUI module SHALL be organized as a directory-based module tree under `src/tui/` with focused submodules. The TUI state layer MUST consume a shared orchestration state model for change progress and execution metadata, while UI-only fields (cursor, view modes, selection state) remain in TUI-owned state. 共有オーケストレーション状態から取り込むイテレーション番号は、既存のTUI表示より小さい値で上書きしてはならない（MUST NOT）。必要に応じてより大きい値を保持し、表示が巻き戻らないようにしなければならない（MUST）。

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

#### Scenario: Iteration number does not regress during refresh
- **GIVEN** the TUI already displays `iteration_number=4` for a change
- **AND** the shared orchestration state reports `apply_count=3`
- **WHEN** the automatic refresh merges shared state into the TUI change list
- **THEN** the TUI keeps `iteration_number=4`

### Requirement: Public API Stability

The TUI module SHALL maintain its public exports.

However, adding new variants to `OrchestratorEvent` and `TuiCommand` MAY be allowed as part of feature additions within this project.

Existing variant semantics, fields, and names MUST remain compatible.

#### Scenario: New variants can be added without breaking existing ones
- **GIVEN** external code imports from the tui module
- **WHEN** new variants are added to `OrchestratorEvent` or `TuiCommand`
- **THEN** existing variants remain available and unchanged
- **AND** the module continues to compile and run within this repository

### Requirement: No Behavioral Changes

TUI refactoring SHALL NOT change runtime behavior.

Key input handling and TuiCommand processing in `run_tui_loop` MAY be split into helper functions for improved readability. However, existing shortcuts, display, and state transition behavior MUST be preserved.

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
The system SHALL provide the ability to dynamically add and remove changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

#### Scenario: Add to queue during execution
- **WHEN** the user selects a change with the Space key in Running mode
- **THEN** the change ID is added to DynamicQueue and will be executed in the next processing cycle

#### Scenario: Remove from queue during execution
- **WHEN** the user changes a [x] change to [@] with Space or @ key in Running mode
- **THEN** the corresponding change ID is removed from DynamicQueue and will not be executed

#### Scenario: Prevent duplicate additions
- **WHEN** attempting to add a change ID that already exists in the queue
- **THEN** the addition is rejected and the queue state remains unchanged

#### Scenario: Remove non-existent ID
- **WHEN** attempting to remove a change ID that does not exist in the queue
- **THEN** no error occurs and the queue state remains unchanged

### Requirement: Queue State Synchronization

The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for resolve completion, and Space/@ queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for queue operations.

However, for `ResolveWait`/`MergeWait` rows, the following SHALL be satisfied:
- Space operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.
- @ operation SHALL toggle only the approval state (`is_approved`) and MUST NOT modify `queue_status` or DynamicQueue. If unapproval results in an unapproved state, `selected` MUST be cleared.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is not a target for queue operations.

#### Scenario: Remove from queue by unapprove
- **WHEN** the user unapproves a queued change with the @ key
- **THEN** the status changes to `QueueStatus::NotQueued` and is removed from DynamicQueue

#### Scenario: Remove from queue with Space key
- **WHEN** the user dequeues a [x] change with the Space key in Running mode
- **THEN** the status changes to `QueueStatus::NotQueued` and is removed from DynamicQueue

#### Scenario: Log removal operations
- **WHEN** a change is removed from DynamicQueue
- **THEN** the removal operation is logged

#### Scenario: Cannot change queue state during ResolveWait
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `ResolveWait`
- **WHEN** the user presses Space or `@`
- **THEN** the change status SHALL remain `ResolveWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space operation toggles only the execution mark

#### Scenario: @ operation during ResolveWait changes only approval state
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `ResolveWait`
- **WHEN** the user presses `@`
- **THEN** the change status SHALL remain `ResolveWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** only the approval state is toggled

#### Scenario: Cannot change queue state during MergeWait
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait`
- **WHEN** the user presses Space
- **THEN** the change status SHALL remain `MergeWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space operation toggles only the execution mark

#### Scenario: @ operation during MergeWait changes only approval state
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait`
- **WHEN** the user presses `@`
- **THEN** the change status SHALL remain `MergeWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** only the approval state is toggled

### Requirement: Event-Driven State Updates

The TUI MUST evaluate `MergeWait` in the 5-second auto-refresh and return it to `Queued` if any of the following conditions are met:

- The corresponding worktree does not exist
- The corresponding worktree exists and the worktree branch is not ahead of base

For auto-released changes that are no longer `MergeWait`, merge resolve operation hints and execution via `M` MUST NOT be performed.

Furthermore, changes that are serialized and in a waiting state for resolve SHALL be retained as `ResolveWait` and MUST NOT be returned to `NotQueued` by auto-refresh.

#### Scenario: Release MergeWait when worktree does not exist
- **GIVEN** a change is in `MergeWait`
- **AND** the corresponding worktree does not exist
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the change status returns to `Queued`

#### Scenario: Release MergeWait for worktree with no commits ahead
- **GIVEN** a change is in `MergeWait`
- **AND** the corresponding worktree exists
- **AND** the worktree branch is not ahead of base
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the change status returns to `Queued`

#### Scenario: Cannot use M for changes released from MergeWait
- **GIVEN** a change has returned from `MergeWait` to `Queued`
- **WHEN** the TUI key hints are rendered
- **THEN** the merge resolve hint via `M` is not displayed

#### Scenario: ResolveWait is retained during auto-refresh
- **GIVEN** a change is in `ResolveWait`
- **AND** resolve is in progress for another change
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the change status remains `ResolveWait`

#### Scenario: Changes with WorkspaceState::Archived are identified as ResolveWait
- **GIVEN** a worktree exists and `detect_workspace_state` returns `WorkspaceState::Archived`
- **AND** the change is not merged (ahead of base)
- **WHEN** the TUI auto-refresh is executed
- **THEN** the change status is displayed as `ResolveWait`
- **AND** queue operations via Space/@ keys are not accepted

### Requirement: Log Entry Structure and Display

TUIログエントリは timestamp、message、color、および任意のコンテキスト情報（change ID、operation、iteration number）を含まなければならない (MUST)。
ログヘッダは利用可能なコンテキスト情報に応じて段階的に表示される。
apply/archive/acceptance/resolve の開始時には、対応する subcommand 文字列が TUI ログに表示されなければならない。
subcommand の出力ログは対応する operation を付与して記録されなければならない。

- Logsビュー（ログパネル）では、operation を持つログは change_id がある場合に iteration があれば `[{change_id}:{operation}:{iteration}]`、iteration がない場合に `[{change_id}:{operation}]` 形式で表示しなければならない。
- 変更一覧のログプレビューでは、operation を持つログは iteration がある場合に `[operation:{iteration}]`、iteration がない場合に `[operation]` 形式で表示し、change_id を表示してはならない。
- change_id を持たない analysis のログ出力は必ず iteration number を含み、ヘッダは `[analysis:{iteration}]` 形式で表示されなければならない。
- auto-scroll が無効な場合、TUI はユーザーが閲覧しているログ範囲を維持し、表示行は新しいログ追加やログバッファのトリミングで移動してはならない。表示行がトリミングされた場合は、最も古い残存ログ行にクランプされなければならず、auto-scroll は自動的に再有効化されてはならない。

#### Scenario: apply/archive/acceptance/resolve の command が表示される
- **GIVEN** change_id が設定され、apply/archive/acceptance/resolve の開始イベントに command が含まれている
- **WHEN** TUI が開始イベントを処理する
- **THEN** ログに `Command:` 行が追加される
- **AND** ログは対応する operation 付きで記録される

#### Scenario: LogsビューのArchiveログヘッダはchange_idとiterationを含む
- **GIVEN** `change_id="test-change"`、`operation="archive"`、`iteration=2` のログエントリが作成される
- **WHEN** TUI が Logs ビューのログを描画する
- **THEN** ログヘッダは `[test-change:archive:2]` として表示される
- **AND** retry の順序が判別できる

#### Scenario: Analysis ログは iteration 付きで表示される
- **GIVEN** `change_id=None`、`operation="analysis"`、`iteration=3` のログエントリが作成される
- **WHEN** TUI が Logs ビューのログを描画する
- **THEN** ログヘッダは `[analysis:3]` として表示される
- **AND** analysis の再実行が区別できる

#### Scenario: auto-scroll が無効なとき表示範囲が固定される
- **GIVEN** ユーザーがログをスクロール済みで auto-scroll が無効になっている
- **WHEN** 新しいログが追加される（必要に応じて古いログがトリミングされる）
- **THEN** 表示範囲は同じログ行を指し続ける
- **AND** 表示範囲がトリミングされた場合、最も古い残存ログ行にクランプされる
- **AND** auto-scroll は自動的に再有効化されない

### Requirement: Reflect tasks.md progress in all states
The TUI MUST continue to display progress obtained from tasks.md even during archive/resolving. If reading tasks.md fails and returns 0/0, the previous progress MUST NOT be overwritten.
In auto-refresh processing, if 0/0 is returned from the active location, the archive location MUST be tried, and if still 0/0, the existing value MUST be retained.

#### Scenario: 0/0 is returned during Archive/Resolving
- **GIVEN** a change is in Archiving or Resolving state
- **AND** the previous progress is not 0/0
- **WHEN** auto-refresh fails to retrieve tasks.md and returns 0/0
- **THEN** the progress display retains the previous value

#### Scenario: Retain progress during auto-refresh immediately after archive move
- **GIVEN** a change is in Archiving state and tasks.md has been moved to the archive location on the worktree
- **AND** the previous progress is not 0/0
- **WHEN** auto-refresh returns 0/0 from `parse_change_with_worktree_fallback`
- **THEN** `parse_archived_change_with_worktree_fallback` is attempted
- **AND** if still 0/0, the existing progress value is retained

### Requirement: Active Change Input Lockout

When `queue_status.is_active()` returns true for a change, Space (@/x) and @ operations MUST NOT be accepted.
In this case, the selection state, approval state, `queue_status`, and DynamicQueue MUST NOT be modified.

#### Scenario: Reject Space operation while is_active
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses Space
- **THEN** the selection mark SHALL remain unchanged
- **AND** the queue status SHALL remain unchanged
- **AND** DynamicQueue SHALL NOT be modified

#### Scenario: Reject @ operation while is_active
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses `@`
- **THEN** the approval state SHALL remain unchanged
- **AND** the queue status SHALL remain unchanged
- **AND** DynamicQueue SHALL NOT be modified

### Requirement: Change List Log Preview

TUI の変更一覧は、各 change の最新ログエントリを右側の空きスペースに単一行のプレビューとして表示しなければならない (MUST)。プレビューにはログの相対時刻（1分未満は `just now`、1分以上は `<n><unit> ago` 形式。例: `2m ago`, `3h ago`。相対時刻の値は切り捨てで丸める）と短縮ヘッダ形式 `[operation:{iteration}]` または `[operation]`、およびメッセージが含まれ、表示幅に収まるように折り返しなしで省略されなければならない。

- 1分以上の相対時刻は最大 2 単位まで表示しなければならない (MUST)。使用する unit は `d` / `h` / `m` とし、表示形式は例として `1d 12h ago`、`3h 20m ago` のように空白区切りで並べる。値は切り捨てで丸める。
- 該当 change にログエントリが存在しない場合、プレビューは表示してはならない (MUST NOT)。
- プレビュー表示に利用可能な幅が 10 文字未満の場合、プレビューは表示してはならない (MUST NOT)。
- 相対時刻の表示は、ログエントリの生成時刻と現在時刻から描画時に算出されなければならず (MUST)、表示は 1 秒単位で更新されなければならない (MUST)。

#### Scenario: 変更一覧に最新ログの相対時刻付きプレビューが表示される
- **GIVEN** ある change に 2分前のログエントリ（`operation="resolve"`、`iteration=1`）が存在する
- **WHEN** TUI が変更一覧を描画する
- **THEN** change 行に `2m ago [resolve:1]` と最新ログメッセージが同じ行で表示される

#### Scenario: 変更一覧はログがない change にプレビューを表示しない
- **GIVEN** ある change にログエントリが存在しない
- **WHEN** TUI が変更一覧を描画する
- **THEN** その change 行にはログプレビューが表示されない

#### Scenario: 変更一覧はプレビュー幅が不足している場合にプレビューを表示しない
- **GIVEN** ある端末幅ではログプレビュー表示に利用可能な幅が 10 文字未満である
- **WHEN** TUI が変更一覧を描画する
- **THEN** 変更一覧にはログプレビューが表示されない

#### Scenario: 変更一覧は最大2単位の相対時刻を表示する
- **GIVEN** ある change に 1日12時間前のログエントリ（`operation="apply"`、`iteration=3`）が存在する
- **WHEN** TUI が変更一覧を描画する
- **THEN** change 行に `1d 12h ago [apply:3]` と最新ログメッセージが同じ行で表示される

#### Scenario: 相対時刻は経過に応じて更新される
- **GIVEN** ある change に 59秒前のログエントリが存在する
- **WHEN** TUI が変更一覧を描画する
- **THEN** change 行の相対時刻は `just now` として表示される
- **WHEN** その後 2 秒経過して TUI が変更一覧を再描画する
- **THEN** change 行の相対時刻は `1m ago` として表示される
