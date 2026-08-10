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

TUI refactoring SHALL NOT change existing keyboard shortcuts, display behavior, remote-client semantics, or state transitions except where explicitly required for shutdown safety.

When local TUI mode exits while local orchestration launched by that TUI is active, the TUI SHALL treat exit as a cancellation boundary for that local work. It MUST cancel the local orchestrator and its invocation-scoped run command scope, wait for bounded scheduler and owned-process cleanup, and prevent any local orchestrator task or owned AI command process group spawned by that run from continuing after TUI cleanup has completed. The local shutdown deadline MUST NOT expire before the scheduler's outer cancellation boundary has had its configured opportunity to complete command cleanup and pending merge/base-lane handling.

If the orchestrator does not finish within that bound, the TUI MUST use the scope retained outside the orchestrator task to forcefully clean and verify retained owned process identities before aborting and joining the orchestrator task. Task abort alone MUST NOT be treated as child-process cleanup evidence.

Remote TUI mode (`--server`) is excluded from local-work cancellation semantics: closing the remote TUI client MUST NOT implicitly stop remote server-side work.

#### Scenario: Local TUI quit cancels active local orchestration

- **GIVEN** `cflx tui` is running in local mode
- **AND** the TUI has started local orchestration work with an invocation-scoped command scope
- **WHEN** the user exits the TUI
- **THEN** the local orchestrator cancellation token and run command scope are cancelled immediately
- **AND** the TUI waits through the bounded scheduler cleanup boundary
- **AND** active local orchestration and its owned AI command process groups are not allowed to continue after TUI cleanup completes

#### Scenario: Timed-out local orchestrator cleanup force-cleans before task abort

- **GIVEN** local TUI cleanup has requested orchestrator and run-command-scope cancellation
- **AND** the orchestrator task does not finish within the bounded outer cleanup period
- **WHEN** the period expires
- **THEN** the TUI uses retained owned process identities to perform forceful cleanup and quiescence verification
- **AND** deterministic cleanable process groups have no surviving members before the TUI aborts and joins the orchestrator task
- **AND** the task is not silently detached by dropping its join handle
- **AND** an operator-visible diagnostic distinguishes forced abort, unconfirmed cleanup, and graceful completion

#### Scenario: Local shutdown does not undercut command cleanup

- **GIVEN** run-owned command cleanup has a bounded graceful-then-forceful deadline
- **AND** the scheduler has a larger outer cancellation boundary
- **WHEN** local TUI shutdown waits for the active run
- **THEN** its grace does not expire earlier than the scheduler outer boundary
- **AND** nested command and merge cleanup consume one bounded cancellation timeline rather than resetting additive full deadlines

#### Scenario: Remote TUI quit does not stop remote work

- **GIVEN** `cflx tui --server <endpoint>` is connected to a remote server
- **AND** remote work may be running on that server
- **WHEN** the user exits the TUI client
- **THEN** the TUI cancels local client tasks such as rendering, refresh, and WebSocket subscription
- **AND** the TUI does not send an implicit stop or force-stop command to the remote server

### Requirement: Dynamic Queue Management

The system SHALL provide explicit services for dynamically adding and removing changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

Execution-mark mutation through Space or bulk `x` SHALL NOT implicitly call these operations. A mark expresses future run-target intent; it does not add, remove, stop, dequeue, or reschedule work in the active run.

#### Scenario: Marking during execution does not add to DynamicQueue

- **GIVEN** a run is active
- **AND** a visible non-terminal change is not marked
- **WHEN** the user marks the change with Space or bulk `x`
- **THEN** its execution mark becomes true
- **AND** no DynamicQueue `push` occurs
- **AND** no scheduler wake or run dispatch occurs

#### Scenario: Unmarking during execution does not remove admitted work

- **GIVEN** a change is already queued or active in the current run
- **AND** the change carries an execution mark
- **WHEN** the user unmarks it with Space or bulk `x`
- **THEN** its execution mark becomes false
- **AND** no DynamicQueue `remove`, cancellation, stop, or dequeue request occurs
- **AND** current-run execution continues unchanged

#### Scenario: Prevent duplicate additions

- **WHEN** an explicit queue service attempts to add an ID that already exists
- **THEN** the addition is rejected and queue state remains unchanged
- **AND** execution-mark state remains unchanged

#### Scenario: Remove non-existent ID

- **WHEN** an explicit queue service attempts to remove an ID that does not exist
- **THEN** no error occurs and queue state remains unchanged
- **AND** execution-mark state remains unchanged

### Requirement: Queue State Synchronization

The system SHALL synchronize displayed queue state with DynamicQueue and reducer queue intent independently from execution marks.

Space and bulk `x` SHALL toggle only process-local execution marks for visible non-terminal rows and MUST NOT modify DynamicQueue, reducer queue intent, retry/resolve intent, active execution, cancellation, or process mode. Queue status MUST NOT synthesize an execution mark, and mark state MUST NOT synthesize queue status.

`ResolveWait` is scheduler-owned resolve retry work and `MergeWait` is merge-resolution work. Space on either row SHALL toggle only the execution mark and MUST NOT modify `queue_status` or DynamicQueue. `@` SHALL remain ignored. The TUI MUST continue to display `ResolveWait` as `resolve pending`.

In parallel mode, once the user explicitly queues a `NotQueued` change through admitted orchestration, refresh-derived reconciliation MUST preserve the queued display state until execution starts, startup is explicitly rejected, or an explicit dequeue occurs. Auto-refresh, reducer synchronization, and eligibility reconciliation MUST NOT regress it to `not queued` before backend analysis or dispatch.

Configured start keys SHALL remain app-level orchestration controls and MUST NOT emit cursor-local `ResolveMerge` or move a cursor `MergeWait` row to `resolve pending`.

At final admission, run control SHALL read one coherent mark snapshot. A worktree-ineligible marked target SHALL reject the complete request with target-specific diagnostics. Other currently non-startable statuses SHALL be excluded from that admission with target-specific diagnostics; if no runnable target remains, admission SHALL reject. Error-mode retry SHALL route only marked retry-eligible error targets and report other marked rows as excluded. Rejection MUST leave no partial queue, scheduler, retry-edge, or mode effect.

#### Scenario: Queue and mark projections remain independent

- **GIVEN** a change is reducer-visible as `queued` but is not execution-marked
- **WHEN** frontend state is synchronized
- **THEN** the row remains queued and unmarked
- **AND** neither projection overwrites the other

#### Scenario: Marking wait and error rows has no workflow side effect

- **GIVEN** a visible non-terminal row is in error, merge wait, resolve pending, or another non-active wait state
- **WHEN** the user toggles its execution mark
- **THEN** only the process-local mark changes
- **AND** no retry, resolve, or queue intent is created

#### Scenario: Worktree-ineligible mark rejects atomically

- **GIVEN** marked targets include a worktree-ineligible change
- **WHEN** the configured start control reaches final admission
- **THEN** the complete request is rejected
- **AND** no scheduler, queue, retry-edge, or mode effect survives
- **AND** the diagnostic identifies that target and reason

#### Scenario: Non-startable status is excluded without blocking runnable work

- **GIVEN** marked targets include one runnable change and one currently non-startable status
- **AND** neither target violates the worktree eligibility fence
- **WHEN** Start reaches final admission
- **THEN** the runnable change is admitted
- **AND** the other target is excluded with target-specific diagnostic detail

#### Scenario: No runnable target rejects

- **GIVEN** every marked target is currently non-startable
- **WHEN** Start reaches final admission
- **THEN** admission is rejected before queue or scheduler effects
- **AND** the diagnostics identify the exclusions

#### Scenario: Configured start key on MergeWait does not resolve cursor row

- **GIVEN** cursor change `alpha` is in `MergeWait`
- **AND** change `beta` is marked runnable work
- **WHEN** a configured start key is pressed
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** normal orchestration MAY admit `beta`

#### Scenario: Default start keys remain app-level controls

- **GIVEN** default start keys are `F5` and `!`
- **AND** the cursor is on a `MergeWait` row
- **WHEN** the user presses `F5` or `!`
- **THEN** the TUI SHALL NOT emit cursor-local `ResolveMerge`
- **AND** normal orchestration MAY proceed for marked runnable work

<!-- Expected canonical result after archive: both historical duplicate `Queue State Synchronization` requirements will converge to this complete contract; archive validation must leave no duplicate legacy Space-to-queue semantics. -->

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
- Logsビューの1行目は timestamp とログヘッダの直後から現在のパネル内側右端までを使用し、表示幅を超えるメッセージの2行目以降はインデントせずにパネル内側の横幅全体を用いて折り返さなければならない。
- Logsビューは保持済みmessageをproducer固定位置で再省略してはならない。`PgUp`、`PgDn`、`Home`、`End` の既存key assignmentは、entry境界だけでなく、viewportより高い単一entry内の折り返し表示行にも到達できなければならない (MUST)。source messageが200文字以上保持されている場合、狭い端末でもこれらの操作により先頭200文字を含む全折り返しsegmentを実際の描画bufferへ表示できなければならない (MUST)。
- Logsビューの表示範囲とnavigation rangeは折り返し後の表示行数で計算され、長文ログの折り返しによって最新ログがauto-scroll viewport外になることがあってはならない。
- auto-scrollが無効な場合、TUIは現在閲覧中のentryとsource-content位置をprocess-local anchorとして維持しなければならない (MUST)。新しいログ追加、filter変更、横幅変更、またはログバッファのtrim後は、そのanchorを現在のfiltered/wrapped sequenceへ決定的に再投影しなければならない (MUST)。anchor対象がtrimされた場合は最も古い残存表示行へclampし、auto-scrollを自動的に再有効化してはならない (MUST NOT)。
- 折り返し、表示行数、末尾省略は Unicode display width で計算され、CJKまたはemojiのUTF-8境界を壊してはならない。

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

#### Scenario: auto-scroll が無効なとき閲覧中content位置が維持される

- **GIVEN** ユーザーがログをスクロール済みで auto-scroll が無効になっている
- **AND** process-local anchorが現在のentryとsource-content位置を指している
- **WHEN** 新しいログ追加、filter変更、横幅変更、またはログバッファtrimにより表示行sequenceが再計算される
- **THEN** anchorは同じentryとsource-content位置を含む新しい表示行へ再投影される
- **AND** anchor対象がtrimされた場合、最も古い残存表示行へclampされる
- **AND** auto-scrollは自動的に再有効化されない

#### Scenario: 幅の広いLogsパネルは追加の内容を表示する

- **GIVEN** 200文字を超える保持済みログメッセージがある
- **WHEN** 同じログを狭いLogsパネルと広いLogsパネルで描画する
- **THEN** 各行はそれぞれの現在のパネル内側幅を使用する
- **AND** 広いパネルの1行は狭いパネルより多くのmessage contentを表示する
- **AND** 200文字等のproducer固定位置に不要なellipsisは表示されない

#### Scenario: 狭く低いLogsパネルで単一entry内を移動できる

- **GIVEN** 200文字以上の保持済みログメッセージがある
- **AND** 1行に200文字を表示できず、entry全体もviewport高に収まらないLogsパネルである
- **WHEN** TUIがログを描画し、ユーザーが `Home`、`PgDn`、`PgUp`、`End` を操作する
- **THEN** 先頭行はtimestampとheader後の利用可能幅を使用する
- **AND** 継続行はインデントせずパネル内側幅全体を使用する
- **AND** operation sequenceを通して少なくとも先頭200文字を含む全折り返しsegmentが実際の描画bufferに現れる
- **AND** `End` またはauto-scroll有効時には最新ログ行が表示範囲から外れない

#### Scenario: Unicodeログの折り返しは表示幅と文字境界を守る

- **GIVEN** CJKとemojiを含む長いログメッセージがある
- **WHEN** TUI が異なる幅のLogsパネルで折り返す
- **THEN** 各表示行はパネル内側のdisplay widthを超えない
- **AND** UTF-8文字境界は壊れない
- **AND** 折り返し部分を結合すると共有安全上限内の保持済みmessage contentが失われていない

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

`queue_status.is_active()` が true の change では、`@` 操作を受け付けてはならない（MUST NOT）。Space は active change でも process-local execution mark のみを変更し、停止要求、cancellation、queue mutation、または即時 `queue_status` 変更を発行してはならない（MUST NOT）。Per-change termination は独立した `K: kill` control を使用しなければならない（SHALL）。

#### Scenario: active change の Space は mark のみ変更する

- **GIVEN** the TUI is in Running mode
- **AND** cursor change has `queue_status.is_active() == true`
- **WHEN** the user presses Space
- **THEN** only its process-local execution mark toggles
- **AND** no stop, cancellation, dequeue, or queue-status mutation occurs
- **AND** current active work continues

#### Scenario: active change で @ 操作は無効

- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses `@`
- **THEN** the approval state remains unchanged
- **AND** the queue_status remains unchanged

### Requirement: Change List Log Preview

The TUI change list MUST display a single-line preview in the remaining space on the right side of each change row. For a change whose display status is `error`, the preview MUST prefer the retained final change-level diagnostic over every buffered log entry and MUST format it as `Error: <diagnostic>`. This error preview MUST remain available independently of bounded log retention. If the status is `error` but no diagnostic is available, the preview MUST use an explicit fallback such as `Error details unavailable` and MUST NOT present an unrelated ordinary log as the failure reason. For every non-error change, the preview MUST display the latest retained log entry and include its relative time (`just now` for less than 1 minute; `<n><unit> ago` for 1 minute or more), the shortened header format `[operation:{iteration}]` or `[operation]`, and the message.

Every preview MUST remain exactly one display line and MUST NOT wrap or increase the change-row height. The renderer MUST use all actual remaining row width and truncate with an ellipsis only when the retained preview does not fit that width. Producer retention is governed by the observability capability; this renderer MUST apply no additional fixed-position cutoff. Truncation MUST use Unicode display width, MUST NOT break UTF-8 character boundaries, and MUST NOT panic for CJK or emoji. Error previews MUST use readable error styling in both focused and unfocused rows.

- For relative times of 1 minute or more on non-error log previews, the display MUST include up to 2 units. Units MUST be `d` / `h` / `m`, formatted as space-separated units such as `1d 12h ago` or `3h 20m ago`. Values MUST be truncated (no rounding up).
- If no log entry exists for a non-error change, the preview MUST NOT be displayed.
- If the available width for the preview is less than 10 characters, the preview MUST NOT be displayed.
- The relative time for a non-error log preview MUST be computed at render time from the log entry creation time and the current time, and the display MUST update at 1-second granularity.

#### Scenario: Wider change row reveals more retained preview content

- **GIVEN** a non-error change has a latest retained log message longer than 200 characters
- **WHEN** the same change row is rendered at two widths that both leave at least 10 preview columns
- **THEN** each preview remains one display line
- **AND** the wider row displays more retained message content than the narrower row
- **AND** ellipsis appears only where that row's actual remaining width cannot contain the retained preview

#### Scenario: Narrow change row never wraps its preview

- **GIVEN** a retained log or error preview does not fit the remaining change-row width
- **WHEN** the TUI renders the Changes list
- **THEN** the preview is truncated to the remaining display width with an ellipsis
- **AND** no continuation line is created
- **AND** the following change or project header retains its expected visual row position

#### Scenario: Unicode preview truncation is width-safe

- **GIVEN** the retained preview contains CJK and emoji
- **AND** the available preview width cannot contain the full value
- **WHEN** the TUI renders the Changes list
- **THEN** the preview remains one line within the available display width
- **AND** truncation does not split a UTF-8 character or panic

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

Changes ビューは、表示中の non-terminal change を対象に、全マーク/全アンマークを1操作で切り替えられなければならない（SHALL）。

この操作は Select、Running、Stopping、Stopped、および Error の全 execution mode で有効でなければならない（SHALL）。warning popup、confirmation、QR、またはその他の overlay が input を所有する場合は overlay がキーを消費し、Changes view の bulk mark を実行してはならない（MUST NOT）。

トグル対象に未マークが1件でも存在する場合は対象を全てマークし、対象が全てマーク済みの場合は全てアンマークしなければならない（SHALL）。bulk mark は execution mark のみを変更し、DynamicQueue、reducer queue intent、retry、resolve、cancellation、scheduler、hook、または process mode を変更してはならない（MUST NOT）。Archived、merged、pushed、および rejected rows は対象外でなければならない（SHALL）。

#### Scenario: 全 execution mode で未マークを全マークする

- **GIVEN** the TUI is in Select, Running, Stopping, Stopped, or Error mode
- **AND** at least one visible non-terminal change is not marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all visible non-terminal changes SHALL be marked
- **AND** no queue or runtime side effect occurs

#### Scenario: すべてマーク済みの場合は全アンマークする

- **GIVEN** all visible non-terminal changes are marked
- **WHEN** the user triggers the bulk toggle in any execution mode
- **THEN** all visible non-terminal changes SHALL be unmarked
- **AND** work already admitted to the current run remains unchanged

#### Scenario: terminal row は bulk 対象外

- **GIVEN** visible rows include archived, merged, pushed, or rejected changes
- **WHEN** the user triggers the bulk toggle
- **THEN** terminal rows are excluded without a mark refusal warning
- **AND** every visible non-terminal row receives the common target mark state

<!-- Expected canonical result after archive: `tui-architecture` will separate execution marks from DynamicQueue, replace active Space stop with K, and converge duplicate synchronization requirements without dropping resolve or queued-display guarantees. -->

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

The system SHALL synchronize displayed queue state with DynamicQueue and reducer queue intent independently from execution marks.

Space and bulk `x` SHALL toggle only process-local execution marks for visible non-terminal rows and MUST NOT modify DynamicQueue, reducer queue intent, retry/resolve intent, active execution, cancellation, or process mode. Queue status MUST NOT synthesize an execution mark, and mark state MUST NOT synthesize queue status.

`ResolveWait` is scheduler-owned resolve retry work and `MergeWait` is merge-resolution work. Space on either row SHALL toggle only the execution mark and MUST NOT modify `queue_status` or DynamicQueue. `@` SHALL remain ignored. The TUI MUST continue to display `ResolveWait` as `resolve pending`.

In parallel mode, once the user explicitly queues a `NotQueued` change through admitted orchestration, refresh-derived reconciliation MUST preserve the queued display state until execution starts, startup is explicitly rejected, or an explicit dequeue occurs. Auto-refresh, reducer synchronization, and eligibility reconciliation MUST NOT regress it to `not queued` before backend analysis or dispatch.

Configured start keys SHALL remain app-level orchestration controls and MUST NOT emit cursor-local `ResolveMerge` or move a cursor `MergeWait` row to `resolve pending`.

At final admission, run control SHALL read one coherent mark snapshot. A worktree-ineligible marked target SHALL reject the complete request with target-specific diagnostics. Other currently non-startable statuses SHALL be excluded from that admission with target-specific diagnostics; if no runnable target remains, admission SHALL reject. Error-mode retry SHALL route only marked retry-eligible error targets and report other marked rows as excluded. Rejection MUST leave no partial queue, scheduler, retry-edge, or mode effect.

#### Scenario: Queue and mark projections remain independent

- **GIVEN** a change is reducer-visible as `queued` but is not execution-marked
- **WHEN** frontend state is synchronized
- **THEN** the row remains queued and unmarked
- **AND** neither projection overwrites the other

#### Scenario: Marking wait and error rows has no workflow side effect

- **GIVEN** a visible non-terminal row is in error, merge wait, resolve pending, or another non-active wait state
- **WHEN** the user toggles its execution mark
- **THEN** only the process-local mark changes
- **AND** no retry, resolve, or queue intent is created

#### Scenario: Worktree-ineligible mark rejects atomically

- **GIVEN** marked targets include a worktree-ineligible change
- **WHEN** the configured start control reaches final admission
- **THEN** the complete request is rejected
- **AND** no scheduler, queue, retry-edge, or mode effect survives
- **AND** the diagnostic identifies that target and reason

#### Scenario: Non-startable status is excluded without blocking runnable work

- **GIVEN** marked targets include one runnable change and one currently non-startable status
- **AND** neither target violates the worktree eligibility fence
- **WHEN** Start reaches final admission
- **THEN** the runnable change is admitted
- **AND** the other target is excluded with target-specific diagnostic detail

#### Scenario: No runnable target rejects

- **GIVEN** every marked target is currently non-startable
- **WHEN** Start reaches final admission
- **THEN** admission is rejected before queue or scheduler effects
- **AND** the diagnostics identify the exclusions

#### Scenario: Configured start key on MergeWait does not resolve cursor row

- **GIVEN** cursor change `alpha` is in `MergeWait`
- **AND** change `beta` is marked runnable work
- **WHEN** a configured start key is pressed
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** normal orchestration MAY admit `beta`

#### Scenario: Default start keys remain app-level controls

- **GIVEN** default start keys are `F5` and `!`
- **AND** the cursor is on a `MergeWait` row
- **WHEN** the user presses `F5` or `!`
- **THEN** the TUI SHALL NOT emit cursor-local `ResolveMerge`
- **AND** normal orchestration MAY proceed for marked runnable work

<!-- Expected canonical result after archive: both historical duplicate `Queue State Synchronization` requirements will converge to this complete contract; archive validation must leave no duplicate legacy Space-to-queue semantics. -->

### Requirement: Local TUI header reports workspace dirty state

The local TUI SHALL observe the Git dirty state of the repository root captured at startup on the existing five-second auto-refresh cadence. A successful observation SHALL classify staged changes, unstaged changes, and untracked files as dirty while excluding ignored files. The TUI header SHALL display a warning-styled `[dirty]` badge only for a known dirty observation and SHALL remove it after a later successful clean observation.

The dirty observation and badge SHALL be process-local presentation state only. They MUST NOT influence reducer state, execution marks, command admission, queue membership, scheduler dispatch, resume routing, acceptance, archive, merge, or any next-action decision.

#### Scenario: Staged change appears in the header

- **GIVEN** the local TUI captured repository root `/repo` at startup
- **AND** `/repo` contains a staged change
- **WHEN** the existing five-second auto-refresh successfully observes Git status
- **THEN** the TUI header displays a red bold `[dirty]` badge after the workspaces badge
- **AND** the existing process-mode, workspaces, and version header content remains visible

#### Scenario: Unstaged change appears in the header

- **GIVEN** the captured repository root contains an unstaged tracked-file change
- **WHEN** the existing auto-refresh successfully observes Git status
- **THEN** the TUI header displays `[dirty]`

#### Scenario: Untracked file appears in the header

- **GIVEN** the captured repository root contains an untracked file
- **AND** repository or user configuration would otherwise suppress untracked status output
- **WHEN** the existing auto-refresh uses the shared dirty-state Git predicate
- **THEN** the TUI header displays `[dirty]`

#### Scenario: Ignored files do not appear as dirty

- **GIVEN** the captured repository root is clean except for ignored files
- **WHEN** the existing auto-refresh successfully observes Git status
- **THEN** the TUI header does not display `[dirty]`

#### Scenario: Successful clean refresh removes the badge

- **GIVEN** the latest successful observation is dirty and the TUI header displays `[dirty]`
- **AND** the workspace is subsequently cleaned
- **WHEN** the next existing five-second auto-refresh successfully observes the clean state
- **THEN** the TUI removes `[dirty]` without restarting
- **AND** no orchestration state changes solely because the badge disappeared

#### Scenario: Failed observation preserves the last successful state

- **GIVEN** the latest successful workspace observation is dirty
- **WHEN** a later Git status observation fails
- **THEN** the TUI preserves the dirty presentation state
- **AND** it does not replace the state with clean
- **AND** it emits a bounded warning without stopping refresh or orchestration

#### Scenario: Unknown initial state makes no cleanliness claim

- **GIVEN** the TUI has not completed a successful workspace dirty observation
- **WHEN** the header is rendered
- **THEN** `[dirty]` is omitted
- **AND** the unknown observation is not treated as clean evidence for any workflow decision

#### Scenario: Refresh remains bound to the captured repository root

- **GIVEN** the local TUI captured repository root `/repo` at startup
- **AND** the process current working directory later changes to `/other`
- **AND** `/repo` is dirty while `/other` is clean
- **WHEN** the existing five-second auto-refresh observes workspace dirty state
- **THEN** the TUI displays `[dirty]` from `/repo`
- **AND** it does not derive the badge from `/other`

#### Scenario: Dirty badge is observability-only

- **GIVEN** two otherwise identical TUI states differ only in their workspace dirty presentation observation
- **WHEN** reducer status, execution marks, command admission, queue routing, resume, acceptance, archive, merge, and next-action behavior are evaluated
- **THEN** both states produce identical workflow behavior
- **AND** only the rendered header badge differs

<!-- Expected canonical result after archive: `tui-architecture` will require an observability-only local TUI dirty badge driven by the captured-root five-second refresh and the shared Git dirty predicate. -->
