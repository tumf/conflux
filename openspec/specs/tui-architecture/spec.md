# tui-architecture Specification

## Purpose
Defines the TUI module structure and architectural patterns.
## Requirements

### Requirement: TUI Module Structure

TUI モジュールは `src/tui/` 配下のディレクトリ構成で整理され、TUI state 層は共有オーケストレーション状態から change の進捗と実行メタデータを取得しなければならない（SHALL）。UI 固有の状態（カーソル、ビュー、選択状態など）は TUI 側で保持する。
共有状態から取り込む iteration は、既に表示されている値より小さい場合に上書きしてはならない。表示された iteration が後退しないよう、より大きい値を維持しなければならない。
さらに、出力イベントにより iteration を更新する際は、現在の `queue_status` に一致するステージのイベントのみを反映し、同一ステージ内で iteration が単調増加となるように更新しなければならない。ステージ開始時は iteration 表示をリセットし、前ステージの値を持ち越してはならない。この更新規則は MUST とする。

Running mode の logs panel が有効な場合、TUI は changes list の現在の表示需要を超える余剰縦領域を logs panel に割り当てなければならない（MUST）。ただし、changes が多く list 領域を必要とする場合、logs panel は既存の 20 行相当の高さを維持しなければならない（MUST）。logs panel が無効な Running mode、および Select mode / Worktree view のレイアウトはこの動的再配分の対象外とする。

Local TUI mode の auto-refresh は、起動時に確定した repository root を基準に active changes と rejected marker rows を読み取らなければならない（MUST）。Auto-refresh は process current working directory の後続変化に依存して change discovery を行ってはならない（MUST NOT）。

Running mode は、auto-refresh により新規 active change が検出され `new_change_count > 0` になった場合、追加された row が現在の changes list viewport 外にあってもユーザーが新規 change の存在を認識できる表示シグナルを出さなければならない（MUST）。この表示シグナルは observability-only であり、queue、scheduler dispatch、resume routing、acceptance、archive、next-action decision の入力として使ってはならない（MUST NOT）。

<!-- Expected canonical result after archive: `tui-architecture` will require local auto-refresh to use captured repo_root for change discovery and require Running mode to surface a new-change signal even when the appended row is off-screen. -->

#### Scenario: Few changes expand logs panel

- **GIVEN** the TUI is rendering Running mode with the logs panel enabled
- **AND** the terminal has more vertical space than the current changes visual rows need
- **WHEN** the Running mode layout is computed
- **THEN** the logs panel receives the surplus vertical space beyond the changes list display need
- **AND** the logs panel is taller than the existing fixed 20-row allocation

#### Scenario: Many changes preserve current logs height

- **GIVEN** the TUI is rendering Running mode with the logs panel enabled
- **AND** the current changes visual rows need the available flexible changes-list area
- **WHEN** the Running mode layout is computed
- **THEN** the logs panel keeps the existing 20-row allocation
- **AND** the changes list uses the remaining vertical space

#### Scenario: Logs disabled layout remains unchanged

- **GIVEN** the TUI is rendering Running mode with the logs panel disabled
- **WHEN** the Running mode layout is computed
- **THEN** only the header, changes list, and status areas are rendered
- **AND** the changes list continues to receive the flexible remaining area

#### Scenario: Local refresh uses captured repository root

- **GIVEN** local TUI mode started from repository root `/repo`
- **AND** the process current working directory later differs from `/repo`
- **AND** `/repo/openspec/changes/new-visible/proposal.md` exists
- **WHEN** the auto-refresh task scans changes
- **THEN** `new-visible` is discovered from `/repo/openspec/changes`
- **AND** discovery does not depend on the later process current working directory

#### Scenario: Running mode surfaces off-screen new change

- **GIVEN** the TUI is in Running mode with the logs panel enabled
- **AND** enough changes exist that a newly appended row may be outside the visible changes-list viewport
- **WHEN** auto-refresh discovers a new active change
- **THEN** Running mode displays a visible new-change signal such as `New: 1`
- **AND** the signal remains visible even if the new row itself is not currently visible
- **AND** the cursor is not moved solely due to the discovery

#### Scenario: New-change signal does not control workflow

- **GIVEN** Running mode displays a new-change signal for change `new-visible`
- **WHEN** scheduler routing, queue membership, resume routing, acceptance, or archive decisions are evaluated
- **THEN** the display signal and any associated TUI log are ignored as workflow-control inputs
- **AND** `new-visible` is not selected or queued unless the user explicitly marks it

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

TUI refactoring SHALL NOT change existing keyboard shortcuts, display behavior, or state transitions except where explicitly required for shutdown safety.

When local TUI mode exits while local orchestration launched by that TUI is active, the TUI SHALL treat exit as a cancellation boundary for that local work. It MUST cancel the local orchestrator, wait for bounded cleanup, and prevent any local orchestrator task spawned by that TUI from continuing after TUI cleanup has completed.

Remote TUI mode (`--server`) is excluded from local-work cancellation semantics: closing the remote TUI client MUST NOT implicitly stop remote server-side work.

<!-- Expected canonical result after archive: `tui-architecture` will require local TUI exit to cancel and stop local orchestrator tasks decisively while preserving remote client quit semantics. -->

#### Scenario: Local TUI quit cancels active local orchestration

- **GIVEN** `cflx tui` is running in local mode
- **AND** the TUI has started local orchestration work
- **WHEN** the user exits the TUI
- **THEN** the local orchestrator cancellation token is cancelled
- **AND** active local orchestration is not allowed to continue after TUI cleanup completes

#### Scenario: Timed-out local orchestrator cleanup aborts the task

- **GIVEN** local TUI cleanup has requested orchestrator cancellation
- **AND** the orchestrator task does not finish within the bounded cleanup grace period
- **WHEN** the grace period expires
- **THEN** the TUI aborts the local orchestrator task
- **AND** the task is not silently detached by dropping its join handle
- **AND** an operator-visible diagnostic distinguishes forced abort from graceful completion

#### Scenario: Remote TUI quit does not stop remote work

- **GIVEN** `cflx tui --server <endpoint>` is connected to a remote server
- **AND** remote work may be running on that server
- **WHEN** the user exits the TUI client
- **THEN** the TUI cancels local client tasks such as rendering, refresh, and WebSocket subscription
- **AND** the TUI does not send an implicit stop or force-stop command to the remote server

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
- **WHEN** the user changes a [x] change to [ ] with the Space key in Running mode
- **THEN** the corresponding change ID is removed from DynamicQueue and will not be executed

#### Scenario: Prevent duplicate additions
- **WHEN** attempting to add a change ID that already exists in the queue
- **THEN** the addition is rejected and the queue state remains unchanged

#### Scenario: Remove non-existent ID
- **WHEN** attempting to remove a change ID that does not exist in the queue
- **THEN** no error occurs and the queue state remains unchanged

### Requirement: Queue State Synchronization

The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for scheduler-owned resolve retry work, and Space queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for normal queue operations.

However, for `ResolveWait`/`MergeWait` rows, the following SHALL be satisfied:
- Space operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.
- @ operation SHALL be ignored and MUST NOT modify any state.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is scheduler-owned retry work and not normal queue work.

In parallel mode, once the user explicitly queues a `NotQueued` change for execution (for example via the configured start key after marking it), refresh-derived state reconciliation MUST preserve the queued display state until one of the following occurs:
- execution for that change actually starts,
- the backend explicitly rejects startup for that change, or
- the user explicitly dequeues the change.

Auto-refresh, reducer display synchronization, and eligibility reconciliation MUST NOT regress such a queued row back to `not queued` before backend analysis/dispatch begins.

The configured start keybindings SHALL be treated as app-level orchestration control and MUST NOT perform cursor-local merge resolve actions. A cursor row in `MergeWait` MUST NOT cause any configured start key to emit `ResolveMerge` or transition that row to `resolve pending`.

<!-- Expected canonical result after archive: `tui-architecture` will generalize the historical F5-specific orchestration control rule to all configured start keybindings. -->

#### Scenario: Configured start key on MergeWait does not resolve cursor row

- **GIVEN** the TUI cursor is on change `alpha`
- **AND** `alpha` is in `MergeWait`
- **AND** change `beta` is marked runnable work in `NotQueued`
- **AND** the resolved TUI start keybindings include `r`
- **WHEN** the user presses `r`
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** `alpha` SHALL NOT transition to `resolve pending` because of `r`
- **AND** normal orchestration start/resume/retry MAY proceed for marked runnable work such as `beta`

#### Scenario: Default F5 remains an app-level start key

- **GIVEN** no TUI config override exists
- **AND** the TUI cursor is on change `alpha` in `MergeWait`
- **WHEN** the user presses `F5`
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** normal orchestration start/resume/retry MAY proceed for marked runnable work

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

Manual resolve completion MUST update the shared reducer-owned display state before the next refresh-derived status synchronization so that successful merge completion is not overwritten by stale `ResolveWait`.

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

#### Scenario: Successful manual resolve is not regressed by refresh
- **GIVEN** a change was previously shown as `ResolveWait`
- **AND** manual resolve completes successfully and the change becomes merged
- **WHEN** the next TUI auto-refresh synchronizes reducer-derived display status
- **THEN** the row remains `Merged` or is removed according to retention rules
- **AND** the row does not return to `resolve pending`

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
`queue_status.is_active()` が true の change では、`@` 操作を受け付けてはならない（MUST NOT）。
`Space` 操作は単体停止要求として受け付けなければならない（SHALL）。`Space` 操作は即時に `selected`/`queue_status` を変更してはならず（SHALL NOT）、停止完了イベントを受信した時点で `selected=false` と `not queued` に遷移させなければならない（SHALL）。

#### Scenario: active change を Space で単体停止する
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses Space
- **THEN** a stop request for that change is issued
- **AND** the change remains in its active queue_status until stop completion
- **AND** after `ChangeStopped` is processed, the change becomes `not queued` and `selected` is cleared

#### Scenario: active change で @ 操作は無効
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses `@`
- **THEN** the approval state remains unchanged
- **AND** the queue_status remains unchanged

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

TUI and scheduler-visible state SHALL classify merge deferrals by first evaluating active resolve/base-mutating lane occupancy and only then evaluating workspace/base dirty state.

If a `MergeDeferred` or merge retry classification occurs while another resolve/base-mutating operation is active, and the deferred change is not the currently resolving change itself, the change SHALL be represented as `ResolveWait` / `resolve pending` and SHALL remain scheduler-owned retry work. Dirty workspace/base evidence observed during the active resolve/base-mutating operation MUST NOT by itself cause manual `MergeWait`.

If no resolve/base-mutating operation is active, dirty workspace/base evidence SHALL be classified as manual `MergeWait` with scheduler-owned `ResolveWait` membership cleared until explicit retry intent is accepted.

If no resolve/base-mutating operation is active and retry preconditions are clean, a scheduler-owned retry MAY be promoted to `Resolving` by the scheduler.

<!-- Expected canonical result after archive: `tui-architecture` will require active resolve/base-mutating occupancy to be evaluated before dirty state when deciding between `resolve pending` and `merge wait`. -->

#### Scenario: Active resolve takes precedence over dirty evidence

- **GIVEN** change `alpha` is currently resolving or otherwise owns the base-mutating lane
- **AND** base/workspace state appears dirty because of `alpha`
- **AND** change `beta` receives merge deferral or retry classification
- **WHEN** the TUI/scheduler classifies `beta`
- **THEN** active resolve/base-mutating occupancy SHALL be evaluated before dirty state
- **AND** `beta` SHALL be represented as `ResolveWait` / `resolve pending`
- **AND** `beta` SHALL NOT be represented as manual `MergeWait` solely because the base/workspace appears dirty

#### Scenario: Dirty without active resolve becomes manual MergeWait

- **GIVEN** no resolve/base-mutating operation is active
- **AND** base/workspace state is dirty or manually blocked
- **AND** change `beta` receives merge retry classification
- **WHEN** the TUI/scheduler classifies `beta`
- **THEN** `beta` SHALL be represented as manual `MergeWait`
- **AND** scheduler-owned `ResolveWait(beta)` SHALL be cleared until explicit retry intent is accepted

#### Scenario: Clean pending retry can start resolving

- **GIVEN** no resolve/base-mutating operation is active
- **AND** change `beta` is scheduler-owned `ResolveWait`
- **AND** retry preconditions for `beta` are clean
- **WHEN** the scheduler evaluates pending base-mutating lane waiters
- **THEN** `beta` MAY transition from `resolve pending` to `resolving`

### Requirement: Bulk Execution Mark Toggle

Changes ビューは、実行マーク可能な change を対象に、全マーク/全アンマークを1操作で切り替えられなければならない（SHALL）。

この操作は Select/Stopped モードでのみ有効で、Running/Stopping/Error では無効でなければならない（SHALL）。

トグル対象に未マークが1件でも存在する場合は対象を全てマークし、対象が全てマーク済みの場合は全てアンマークしなければならない（SHALL）。

#### Scenario: 未マークが残っている場合は全マークする
- **GIVEN** the TUI is in select mode
- **AND** at least one eligible change is not marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all eligible changes SHALL be marked

#### Scenario: すべてマーク済みの場合は全アンマークする
- **GIVEN** the TUI is in stopped mode
- **AND** all eligible changes are marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all eligible changes SHALL be unmarked

### Requirement: リモートデータソース対応
TUI は `--server` が指定された場合、ローカルの共有状態ではなくリモート API をデータソースとして使用しなければならない（MUST）。

#### Scenario: リモートデータソースに切り替わる
- **GIVEN** `--server` が指定されている
- **WHEN** TUI が change 一覧を構築する
- **THEN** ローカルの change 一覧を読み込まない
- **AND** リモート API の結果を使用する

### Requirement: プロジェクト単位のグルーピング表示

TUI は `--server` 指定時の change 一覧をプロジェクト単位でグルーピングして表示しなければならない（SHALL）。各プロジェクトは見出し行として表示し、見出し行は選択や操作の対象にしてはならない（MUST NOT）。各 change 行はプロジェクト名を重複表示せず、change_id のみを表示しなければならない（SHALL）。カーソル移動と選択/実行の操作は change 行のみを対象にしなければならない（SHALL）。

#### Scenario: プロジェクトごとに表示が区切られる
- **GIVEN** サーバに 2 つのプロジェクトが登録されている
- **WHEN** TUI が `--server` 指定で change 一覧を表示する
- **THEN** change 一覧はプロジェクト見出しで区切られて表示される
- **AND** 各 change 行には change_id のみが表示される

#### Scenario: 見出し行は選択対象にならない
- **GIVEN** サーバに 2 つのプロジェクトが登録されている
- **WHEN** ユーザーが ↑↓ でカーソル移動し、Space で選択を切り替える
- **THEN** カーソルは change 行にのみ移動する
- **AND** 見出し行は選択や操作の対象にならない

### Requirement: リモート更新の購読
TUI は `--server` が指定された場合、リモートサーバの状態更新を購読し、既存の iteration 非後退ルールに従って反映しなければならない（MUST）。

リモート更新にはログイベントが含まれる場合があり、TUI はログパネルと change 行のログプレビューに反映しなければならない（MUST）。

#### Scenario: 古い iteration で上書きしない
- **GIVEN** TUI が `iteration_number=3` を表示している
- **WHEN** リモート更新で `iteration_number=2` が届く
- **THEN** TUI は `iteration_number=3` を保持する

#### Scenario: リモートログがログパネルに表示される
- **GIVEN** TUI が `--server` でリモートに接続している
- **WHEN** WebSocket でログイベントが届く
- **THEN** TUI のログパネルにログが表示される

### Requirement: Focused blocked rows remain readable

When the TUI displays a change row that is visually blocked or dimmed to communicate a restriction, the focused-row highlight SHALL preserve readable contrast for the row's primary text and badges. The blocked/dimmed meaning and the focused/cursor meaning MUST remain visually distinguishable from each other.

This requirement applies to the Changes list in both Select and Running views, including rows that are marked as parallel-ineligible because the Git working tree has uncommitted or untracked files.

#### Scenario: Focused uncommitted row in Select view remains legible

- **GIVEN** the TUI is in the Changes list Select view
- **AND** a change row is marked as parallel-ineligible because uncommitted or untracked files exist
- **AND** the cursor is on that row
- **WHEN** the row is rendered
- **THEN** the change ID and blocked badge remain readable
- **AND** the row still appears visually blocked compared with a normal actionable row
- **AND** the cursor/focus state remains visually apparent

#### Scenario: Focused uncommitted row in Running view remains legible

- **GIVEN** the TUI is in the Changes list Running view
- **AND** a change row is marked as parallel-ineligible because uncommitted or untracked files exist
- **AND** the cursor is on that row
- **WHEN** the row is rendered
- **THEN** the change ID, badges, and progress/status text remain readable
- **AND** the blocked state remains distinguishable from the focus state

#### Scenario: Unfocused blocked rows remain visually de-emphasized

- **GIVEN** the TUI displays a blocked or dimmed change row
- **AND** the cursor is on a different row
- **WHEN** the list is rendered
- **THEN** the blocked row remains visually de-emphasized relative to the focused row
- **AND** the contrast fix does not make blocked rows appear like normal actionable rows

### Requirement: MergeDeferred の待ち状態判定

TUI and scheduler-visible state SHALL classify merge deferrals by first evaluating active resolve/base-mutating lane occupancy and only then evaluating workspace/base dirty state.

If a `MergeDeferred` or merge retry classification occurs while another resolve/base-mutating operation is active, and the deferred change is not the currently resolving change itself, the change SHALL be represented as `ResolveWait` / `resolve pending` and SHALL remain scheduler-owned retry work. Dirty workspace/base evidence observed during the active resolve/base-mutating operation MUST NOT by itself cause manual `MergeWait`.

If no resolve/base-mutating operation is active, dirty workspace/base evidence SHALL be classified as manual `MergeWait` with scheduler-owned `ResolveWait` membership cleared until explicit retry intent is accepted.

If no resolve/base-mutating operation is active and retry preconditions are clean, a scheduler-owned retry MAY be promoted to `Resolving` by the scheduler.

<!-- Expected canonical result after archive: `tui-architecture` will require active resolve/base-mutating occupancy to be evaluated before dirty state when deciding between `resolve pending` and `merge wait`. -->

#### Scenario: Active resolve takes precedence over dirty evidence

- **GIVEN** change `alpha` is currently resolving or otherwise owns the base-mutating lane
- **AND** base/workspace state appears dirty because of `alpha`
- **AND** change `beta` receives merge deferral or retry classification
- **WHEN** the TUI/scheduler classifies `beta`
- **THEN** active resolve/base-mutating occupancy SHALL be evaluated before dirty state
- **AND** `beta` SHALL be represented as `ResolveWait` / `resolve pending`
- **AND** `beta` SHALL NOT be represented as manual `MergeWait` solely because the base/workspace appears dirty

#### Scenario: Dirty without active resolve becomes manual MergeWait

- **GIVEN** no resolve/base-mutating operation is active
- **AND** base/workspace state is dirty or manually blocked
- **AND** change `beta` receives merge retry classification
- **WHEN** the TUI/scheduler classifies `beta`
- **THEN** `beta` SHALL be represented as manual `MergeWait`
- **AND** scheduler-owned `ResolveWait(beta)` SHALL be cleared until explicit retry intent is accepted

#### Scenario: Clean pending retry can start resolving

- **GIVEN** no resolve/base-mutating operation is active
- **AND** change `beta` is scheduler-owned `ResolveWait`
- **AND** retry preconditions for `beta` are clean
- **WHEN** the scheduler evaluates pending base-mutating lane waiters
- **THEN** `beta` MAY transition from `resolve pending` to `resolving`

### Requirement: TUI Module Structure

TUI モジュールは `src/tui/` 配下のディレクトリ構成で整理され、TUI state 層は共有オーケストレーション状態から change の進捗と実行メタデータを取得しなければならない（SHALL）。UI 固有の状態（カーソル、ビュー、選択状態など）は TUI 側で保持する。
共有状態から取り込む iteration は、既に表示されている値より小さい場合に上書きしてはならない。表示された iteration が後退しないよう、より大きい値を維持しなければならない。
さらに、出力イベントにより iteration を更新する際は、現在の `queue_status` に一致するステージのイベントのみを反映し、同一ステージ内で iteration が単調増加となるように更新しなければならない。ステージ開始時は iteration 表示をリセットし、前ステージの値を持ち越してはならない。この更新規則は MUST とする。

Running mode の logs panel が有効な場合、TUI は changes list の現在の表示需要を超える余剰縦領域を logs panel に割り当てなければならない（MUST）。ただし、changes が多く list 領域を必要とする場合、logs panel は既存の 20 行相当の高さを維持しなければならない（MUST）。logs panel が無効な Running mode、および Select mode / Worktree view のレイアウトはこの動的再配分の対象外とする。

Local TUI mode の auto-refresh は、起動時に確定した repository root を基準に active changes と rejected marker rows を読み取らなければならない（MUST）。Auto-refresh は process current working directory の後続変化に依存して change discovery を行ってはならない（MUST NOT）。

Running mode は、auto-refresh により新規 active change が検出され `new_change_count > 0` になった場合、追加された row が現在の changes list viewport 外にあってもユーザーが新規 change の存在を認識できる表示シグナルを出さなければならない（MUST）。この表示シグナルは observability-only であり、queue、scheduler dispatch、resume routing、acceptance、archive、next-action decision の入力として使ってはならない（MUST NOT）。

<!-- Expected canonical result after archive: `tui-architecture` will require local auto-refresh to use captured repo_root for change discovery and require Running mode to surface a new-change signal even when the appended row is off-screen. -->

#### Scenario: Few changes expand logs panel

- **GIVEN** the TUI is rendering Running mode with the logs panel enabled
- **AND** the terminal has more vertical space than the current changes visual rows need
- **WHEN** the Running mode layout is computed
- **THEN** the logs panel receives the surplus vertical space beyond the changes list display need
- **AND** the logs panel is taller than the existing fixed 20-row allocation

#### Scenario: Many changes preserve current logs height

- **GIVEN** the TUI is rendering Running mode with the logs panel enabled
- **AND** the current changes visual rows need the available flexible changes-list area
- **WHEN** the Running mode layout is computed
- **THEN** the logs panel keeps the existing 20-row allocation
- **AND** the changes list uses the remaining vertical space

#### Scenario: Logs disabled layout remains unchanged

- **GIVEN** the TUI is rendering Running mode with the logs panel disabled
- **WHEN** the Running mode layout is computed
- **THEN** only the header, changes list, and status areas are rendered
- **AND** the changes list continues to receive the flexible remaining area

#### Scenario: Local refresh uses captured repository root

- **GIVEN** local TUI mode started from repository root `/repo`
- **AND** the process current working directory later differs from `/repo`
- **AND** `/repo/openspec/changes/new-visible/proposal.md` exists
- **WHEN** the auto-refresh task scans changes
- **THEN** `new-visible` is discovered from `/repo/openspec/changes`
- **AND** discovery does not depend on the later process current working directory

#### Scenario: Running mode surfaces off-screen new change

- **GIVEN** the TUI is in Running mode with the logs panel enabled
- **AND** enough changes exist that a newly appended row may be outside the visible changes-list viewport
- **WHEN** auto-refresh discovers a new active change
- **THEN** Running mode displays a visible new-change signal such as `New: 1`
- **AND** the signal remains visible even if the new row itself is not currently visible
- **AND** the cursor is not moved solely due to the discovery

#### Scenario: New-change signal does not control workflow

- **GIVEN** Running mode displays a new-change signal for change `new-visible`
- **WHEN** scheduler routing, queue membership, resume routing, acceptance, or archive decisions are evaluated
- **THEN** the display signal and any associated TUI log are ignored as workflow-control inputs
- **AND** `new-visible` is not selected or queued unless the user explicitly marks it

### Requirement: Queue State Synchronization

The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for scheduler-owned resolve retry work, and Space queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for normal queue operations.

However, for `ResolveWait`/`MergeWait` rows, the following SHALL be satisfied:
- Space operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.
- @ operation SHALL be ignored and MUST NOT modify any state.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is scheduler-owned retry work and not normal queue work.

In parallel mode, once the user explicitly queues a `NotQueued` change for execution (for example via the configured start key after marking it), refresh-derived state reconciliation MUST preserve the queued display state until one of the following occurs:
- execution for that change actually starts,
- the backend explicitly rejects startup for that change, or
- the user explicitly dequeues the change.

Auto-refresh, reducer display synchronization, and eligibility reconciliation MUST NOT regress such a queued row back to `not queued` before backend analysis/dispatch begins.

The configured start keybindings SHALL be treated as app-level orchestration control and MUST NOT perform cursor-local merge resolve actions. A cursor row in `MergeWait` MUST NOT cause any configured start key to emit `ResolveMerge` or transition that row to `resolve pending`.

<!-- Expected canonical result after archive: `tui-architecture` will generalize the historical F5-specific orchestration control rule to all configured start keybindings. -->

#### Scenario: Configured start key on MergeWait does not resolve cursor row

- **GIVEN** the TUI cursor is on change `alpha`
- **AND** `alpha` is in `MergeWait`
- **AND** change `beta` is marked runnable work in `NotQueued`
- **AND** the resolved TUI start keybindings include `r`
- **WHEN** the user presses `r`
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** `alpha` SHALL NOT transition to `resolve pending` because of `r`
- **AND** normal orchestration start/resume/retry MAY proceed for marked runnable work such as `beta`

#### Scenario: Default F5 remains an app-level start key

- **GIVEN** no TUI config override exists
- **AND** the TUI cursor is on change `alpha` in `MergeWait`
- **WHEN** the user presses `F5`
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** normal orchestration start/resume/retry MAY proceed for marked runnable work
