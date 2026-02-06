# tui-architecture Specification

## Purpose
Defines the TUI module structure and architectural patterns.
## Requirements
### Requirement: TUI Module Structure

TUI モジュールは `src/tui/` 配下のディレクトリ構成で整理され、TUI state 層は共有オーケストレーション状態から change の進捗と実行メタデータを取得しなければならない（SHALL）。UI 固有の状態（カーソル、ビュー、選択状態など）は TUI 側で保持する。
共有状態から取り込む iteration は、既に表示されている値より小さい場合に上書きしてはならない。表示された iteration が後退しないよう、より大きい値を維持しなければならない。
さらに、出力イベントにより iteration を更新する際は、現在の `queue_status` に一致するステージのイベントのみを反映し、同一ステージ内で iteration が単調増加となるように更新しなければならない。ステージ開始時は iteration 表示をリセットし、前ステージの値を持ち越してはならない。この更新規則は MUST とする。

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
- **AND** `terminal.rs` contains only terminal execution helpers
- **AND** `worktrees.rs` contains only worktree-related helpers

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

#### Scenario: Output events do not regress iteration within a stage
- **GIVEN** the TUI displays `queue_status=Archiving` and `iteration_number=2`
- **WHEN** an older `ArchiveOutput` event with `iteration=1` arrives
- **THEN** the TUI keeps `iteration_number=2`

#### Scenario: Output events from other stages do not overwrite iteration
- **GIVEN** the TUI displays `queue_status=Resolving` and `iteration_number=2`
- **WHEN** an `ApplyOutput` event arrives for the same change
- **THEN** the TUI keeps `iteration_number=2`

#### Scenario: Stage transition resets iteration display
- **GIVEN** the TUI displays `queue_status=Applying` and `iteration_number=3`
- **WHEN** `ArchiveStarted` is handled for the same change
- **THEN** the TUI clears the iteration display for the new stage

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

The TUI SHALL maintain a FIFO resolve wait queue for manual resolve operations triggered while another resolve is in progress.

When the user presses `M` on a `MergeWait` change while resolve is in progress, the change SHALL transition to `ResolveWait` and be enqueued (deduplicated).

When `ResolveCompleted` is received and the resolve wait queue is not empty, the TUI SHALL dequeue the next change and start its resolve immediately.

When `ResolveFailed` is received, the TUI SHALL NOT auto-start the next resolve; queued changes remain in `ResolveWait` until user action resumes.

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

#### Scenario: resolve 実行中の `M` は待ち行列へ追加される
- **GIVEN** a resolve operation is in progress
- **AND** the user presses `M` on a change in `MergeWait`
- **WHEN** the TUI processes the key event
- **THEN** the change status SHALL transition to `ResolveWait`
- **AND** the change_id SHALL be enqueued for resolve

#### Scenario: ResolveCompleted は次の待ち行列を開始する
- **GIVEN** the resolve wait queue has at least one change_id
- **AND** a resolve operation completes
- **WHEN** `ResolveCompleted` is processed
- **THEN** the next change_id SHALL be dequeued and its resolve started

#### Scenario: ResolveFailed は自動開始しない
- **GIVEN** the resolve wait queue has at least one change_id
- **AND** a resolve operation fails
- **WHEN** `ResolveFailed` is processed
- **THEN** the next resolve SHALL NOT start automatically

### Requirement: Log Entry Structure and Display

TUIログエントリは timestamp、message、color、および任意のコンテキスト情報（change ID、operation、iteration number）を含まなければならない (MUST)。
ログヘッダは利用可能なコンテキスト情報に応じて段階的に表示される。
apply/archive/acceptance/resolve の開始時には、対応する subcommand 文字列が TUI ログに表示されなければならない。
subcommand の出力ログは対応する operation を付与して記録されなければならない。

- Logsビュー（ログパネル）では、operation を持つログは change_id がある場合に iteration があれば `[{change_id}:{operation}:{iteration}]`、iteration がない場合に `[{change_id}:{operation}]` 形式で表示しなければならない。
- 変更一覧のログプレビューでは、operation を持つログは iteration がある場合に `[operation:{iteration}]`、iteration がない場合に `[operation]` 形式で表示し、change_id を表示してはならない。
- change_id を持たない analysis のログ出力は必ず iteration number を含み、ヘッダは `[analysis:{iteration}]` 形式で表示されなければならない。
- Logsビューで表示幅を超えるメッセージは、timestamp とログヘッダの幅を維持したインデントで折り返し表示されなければならない。
- Logsビューの表示範囲は折り返し後の表示行数で計算され、長文ログの折り返しによって最新ログが画面外になることがあってはならない。
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

#### Scenario: 長文ログの折り返しでも表示行がずれない
- **GIVEN** Logsビューに表示幅を超える長文ログが含まれている
- **WHEN** TUI が Logs ビューのログを描画する
- **THEN** 折り返し行は timestamp とヘッダ幅を維持したインデントで表示される
- **AND** 最新ログが表示範囲から外れない

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

The TUI change list MUST display a single-line preview of the latest log entry for each change in the remaining space on the right side of the row. The preview MUST include the log relative time (`just now` for less than 1 minute; `<n><unit> ago` for 1 minute or more, e.g., `2m ago`, `3h ago`, with values truncated (no rounding up)), the shortened header format `[operation:{iteration}]` or `[operation]`, and the message. The preview MUST be truncated without wrapping to fit within the available display width. Truncation MUST NOT break Unicode character boundaries and MUST NOT panic, even when the message contains CJK characters or emoji.

- For relative times of 1 minute or more, the display MUST include up to 2 units. Units MUST be `d` / `h` / `m`, formatted as space-separated units such as `1d 12h ago` or `3h 20m ago`. Values MUST be truncated (no rounding up).
- If no log entry exists for the change, the preview MUST NOT be displayed.
- If the available width for the preview is less than 10 characters, the preview MUST NOT be displayed.
- The relative time MUST be computed at render time from the log entry creation time and the current time, and the display MUST update at 1-second granularity.

#### Scenario: Change list displays preview with relative time for latest log
- **GIVEN** a change has a log entry from 2 minutes ago (`operation="resolve"`, `iteration=1`)
- **WHEN** the TUI renders the change list
- **THEN** the change row displays `2m ago [resolve:1]` and the latest log message on the same line

#### Scenario: Change list does not display preview when no logs exist
- **GIVEN** a change has no log entries
- **WHEN** the TUI renders the change list
- **THEN** the change row does not display a log preview

#### Scenario: Change list does not display preview when preview width is insufficient
- **GIVEN** the available width for the log preview is less than 10 characters
- **WHEN** the TUI renders the change list
- **THEN** the change list does not display a log preview

#### Scenario: Change list displays up to two units for relative time
- **GIVEN** a change has a log entry from 1 day and 12 hours ago (`operation="apply"`, `iteration=3`)
- **WHEN** the TUI renders the change list
- **THEN** the change row displays `1d 12h ago [apply:3]` and the latest log message on the same line

#### Scenario: Relative time updates as time elapses
- **GIVEN** a change has a log entry from 59 seconds ago
- **WHEN** the TUI renders the change list
- **THEN** the change row displays `just now` as the relative time
- **WHEN** 2 seconds pass and the TUI re-renders the change list
- **THEN** the change row displays `1m ago` as the relative time

#### Scenario: Log preview truncation is Unicode-safe for Japanese text
- **GIVEN** the latest log message for a change contains Japanese text (e.g., `追記済みです。`)
- **AND** the available preview width is insufficient to display the full message
- **WHEN** the TUI renders the change list
- **THEN** the log preview is truncated without breaking Unicode character boundaries
- **AND** the TUI continues rendering without panicking

### Requirement: MergeDeferred の待ち状態判定
TUI は `MergeDeferred` を受信したとき、resolve 実行中であり対象 change が現在 resolve 中の change ではない場合、対象 change を `ResolveWait` として扱い、resolve 待ち行列に追加しなければならない（SHALL）。
resolve 実行中で対象 change が現在 resolve 中の change と同一である場合、対象 change は `Resolving` のまま維持され、resolve 待ち行列に追加されてはならない（SHALL NOT）。
resolve が実行中でない場合、対象 change は `MergeWait` のまま保持されなければならない（SHALL）。

#### Scenario: resolve 実行中の MergeDeferred は ResolveWait になる
- **GIVEN** resolve 操作が進行中である
- **AND** change A が `MergeDeferred` を受信する
- **AND** change A は現在 resolve 中の change ではない
- **WHEN** TUI がイベントを処理する
- **THEN** change A のステータスは `ResolveWait` となる
- **AND** change A の change_id が resolve 待ち行列に追加される
- **AND** 表示語彙は `resolve pending` となる

#### Scenario: resolve 実行中の MergeDeferred が現在 resolve 中の change の場合は自己キューしない
- **GIVEN** resolve 操作が進行中である
- **AND** change A が現在 resolve 中の change である
- **AND** change A が `MergeDeferred` を受信する
- **WHEN** TUI がイベントを処理する
- **THEN** change A のステータスは `Resolving` のまま維持される
- **AND** change A の change_id は resolve 待ち行列に追加されない

#### Scenario: resolve 非実行時の MergeDeferred は MergeWait を維持する
- **GIVEN** resolve 操作が進行中ではない
- **AND** change A が `MergeDeferred` を受信する
- **WHEN** TUI がイベントを処理する
- **THEN** change A のステータスは `MergeWait` のまま維持される
