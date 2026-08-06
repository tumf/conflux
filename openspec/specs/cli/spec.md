# cli Specification

## Purpose
Defines CLI commands, subcommands, flags, and their behaviors for the cflx binary.
## Requirements

### Requirement: Subcommand Structure

CLI SHALL have a subcommand structure that supports future command extensions. Bare invocation SHALL launch local TUI. Bare local TUI SHALL accept the same opt-in cumulative upstream integration options as explicit local `tui`: value-less `-u` and `--integrate-upstream` select `origin`, a named remote is accepted only as `--integrate-upstream=<remote>`, and enablement requires `--upstream-verify-command <command>`.

When `--push [remote]` is provided on the top-level TUI entrypoint without an explicit subcommand, the CLI SHALL launch local TUI with push post-archive mode configured for local parallel execution. Upstream integration and push post-archive mode MUST remain mutually exclusive. Top-level upstream integration and `--push` MUST NOT be accepted with `--server` because remote-client TUI does not own the local cumulative base.

#### Scenario: run without subcommand

- **WHEN** user runs `cflx` without arguments
- **THEN** the interactive local TUI is launched
- **AND** the change list is displayed in selection mode
- **AND** upstream integration is disabled

#### Scenario: bare TUI enables upstream integration

- **WHEN** user runs `cflx -u --upstream-verify-command '<command>'`
- **THEN** the interactive TUI is launched in local cumulative parallel mode
- **AND** TUI orchestration receives the same upstream runtime configuration as `cflx run -u`
- **AND** selected remote is `origin`

#### Scenario: bare TUI accepts explicit upstream remote

- **WHEN** user runs `cflx --integrate-upstream=upstream --upstream-verify-command '<command>'`
- **THEN** local TUI upstream integration selects remote `upstream`
- **AND** the option does not configure push post-archive mode

#### Scenario: bare TUI rejects incompatible publication modes

- **WHEN** user combines upstream integration with `--push` or `--server`
- **THEN** TUI orchestration does not start
- **AND** the CLI identifies the incompatible options before repository mutation

#### Scenario: run with unknown subcommand

- **WHEN** user runs with a non-existent subcommand
- **THEN** an error message with available subcommands is displayed

### Requirement: run Subcommand

The `run` subcommand SHALL execute explicit targets through cumulative worktree orchestration. Push post-archive mode and upstream integration remain mutually exclusive; upstream integration SHALL be rejected for unsupported remote-client or server surfaces before work starts.

#### Scenario: run uses explicit targets

- **WHEN** user provides `--all`, positional change IDs, or legacy `--change` values
- **THEN** targets are normalized and dispatched through worktree orchestration
- **AND** no execution-mode selection is required

### Requirement: Orchestration loop runs apply and archive

The orchestration loop SHALL run the configured Apply and Acceptance commands and parse their machine-readable outcomes. It SHALL continue to accept `gated` and legacy `blocked` acceptance tokens as compatibility input, but token spelling alone SHALL NOT determine operator-facing lifecycle status.

Conflux SHALL classify validated non-repository prerequisite evidence as `blocked`. It SHALL classify no-progress, repeated-finding, and exhausted-retry execution stops as `stalled`. Invalid bare compatibility blocker input SHALL use bounded protocol correction and SHALL set neither lifecycle state without sufficient evidence.

#### Scenario: Structured external prerequisite displays blocked

- **GIVEN** Apply or Acceptance emits complete structured evidence for a non-repository prerequisite
- **WHEN** the orchestration loop validates and classifies the result
- **THEN** the change lifecycle status is `blocked`
- **AND** the status detail identifies the blocker kind as external

#### Scenario: No-progress execution displays stalled

- **GIVEN** a change makes no semantic progress, repeats a finding, or exhausts its retry policy
- **WHEN** the orchestration loop finalizes the execution hold
- **THEN** the lifecycle status is `stalled`
- **AND** it is not presented as a dependency or external wait

#### Scenario: Legacy token remains input compatibility only

- **GIVEN** Acceptance emits legacy `blocked` or `gated`
- **WHEN** the orchestration loop parses the verdict
- **THEN** the token remains accepted as compatibility syntax
- **AND** complete structured facts are still required for external `blocked`
- **AND** bare input follows bounded protocol correction

### Requirement: Default TUI Launch

When launched without a subcommand, the interactive TUI SHALL be displayed. The non-interactive `run` subcommand SHALL require explicit target selection and SHALL NOT use bare `cflx run` as a backward-compatible all-changes shortcut.

<!-- Expected canonical result after archive: Default TUI Launch will preserve bare `cflx` TUI launch while removing the bare `cflx run` compatibility scenario. -->

#### Scenario: Launch without subcommand

- **WHEN** user runs `cflx` without arguments
- **THEN** the interactive TUI is launched
- **AND** the change list is displayed in selection mode

#### Scenario: Launch with run subcommand requires targets

- **WHEN** user runs `cflx run` without explicit targets
- **THEN** the orchestration loop is not executed
- **AND** the command exits with guidance to run `cflx run --all` or `cflx run <change-id>...`

### Requirement: Change Selection Mode

At TUI launch, the change selection mode SHALL be displayed, allowing users to select changes for processing.

#### Scenario: Exit
- **WHEN** user presses `Ctrl+C`
- **THEN** TUI exits and the terminal is restored to its original state

### Requirement: Start Execution of Selected Changes

When F5 key is pressed in selection mode, processing of selected changes SHALL begin.

#### Scenario: Start execution with F5 key

- **WHEN** user presses F5 key
- **AND** one or more changes are selected
- **THEN** TUI switches to running mode
- **AND** selected changes are added to the queue

#### Scenario: F5 key with no selection

- **WHEN** user presses F5 key
- **AND** no changes are selected
- **THEN** execution does not start
- **AND** a warning message is displayed

### Requirement: Running Mode Dashboard

TUI は Running モードでダッシュボード形式の UI を表示しなければならない（SHALL）。
正常完了時は Ready 表示に戻り、停止要求がない限り Stopped へ遷移してはならない。

TUI が shared reducer の display snapshot を `AppState` に同期する場合、Running mode の in-flight 状態を表す execution lifecycle events を reducer に反映してから display snapshot を適用しなければならない（MUST）。これにより、`ChangesRefreshed` 後も active display status と header count が stale reducer snapshot によって失われてはならない（MUST NOT）。

ヘッダーステータスは現在のオーケストレーション活動を表示し、内部の停止後再開制御状態を新しい実行中ステータスとして公開してはならない（MUST NOT）。`AppExecutionMode::Select` と `AppExecutionMode::Stopped` は `Ready`、`Running` は `Running` または `Running <count>`、`Stopping` は `Stopping` を表示する。`Error` は既存どおりステータスラベルを表示しない。内部 Stopped mode は resume routing と controls のために維持し、Header projectionによって変更してはならない（MUST NOT）。

#### Scenario: Display on processing completion
- **WHEN** すべての queued change が処理完了する
- **THEN** ヘッダーステータスが "Ready" に切り替わる
- **AND** TUI は Select（Ready）モードに戻る
- **AND** ステータスパネルは進捗と経過時間のみを表示する
- **AND** `Ctrl+C` で終了できるよう表示を維持する

#### Scenario: Running mode header shows processing count
- **GIVEN** TUI が Running モードである
- **WHEN** 1 件以上の change が in-flight 状態（Applying/Accepting/Archiving/Resolving）である
- **THEN** ヘッダーは "Running <count>" を表示し、<count> は in-flight change の件数になる
- **AND** queued の change は <count> に含めない

#### Scenario: Reducer display sync preserves active header count
- **GIVEN** TUI が Running モードである
- **AND** shared reducer display snapshot が `AppState` の表示状態に同期される
- **WHEN** `ApplyStarted`, `AcceptanceStarted`, `ArchiveStarted`, or `ResolveStarted` が発生し、その後 `ChangesRefreshed` が発生する
- **THEN** 当該 change の表示状態は in-flight 状態として保持される
- **AND** ヘッダーは active change 数を `Running <count>` として表示し続ける
- **AND** queued のみの change は <count> に含めない

#### Scenario: Stopped mode header projects Ready
- **GIVEN** TUI の内部 execution mode が Stopped である
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーは cyan の `Ready` ステータスを表示する
- **AND** ヘッダーは `Stopped` ステータスを表示しない
- **AND** 内部 execution mode は Stopped のまま維持される

#### Scenario: Error mode header remains unlabeled
- **GIVEN** TUI が Error モードである
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーはステータスラベルを表示しない

### Requirement: TUI Layout Structure

The TUI SHALL display appropriate layout for Stopping and Stopped modes in addition to existing modes. Stopped mode SHALL use the Ready header projection while retaining stopped-mode resume controls.

#### Scenario: Stopping mode layout

- **WHEN** TUI is in Stopping mode
- **THEN** header displays "Stopping..." status in yellow
- **AND** current processing panel shows "Completing..."
- **AND** ログパネルが有効な場合は停止メッセージを含むログパネルが表示される
- **AND** ログパネルが無効な場合でも停止メッセージはログに記録される

#### Scenario: Stopped mode layout

- **WHEN** TUI is in Stopped mode
- **THEN** header displays "Ready" status in cyan
- **AND** status panel shows summary of completed/queued changes
- **AND** footer shows available actions (F5: resume, q: quit)
- **AND** rendering does not change the internal Stopped mode

### Requirement: Auto-refresh Feature

TUI SHALL periodically auto-refresh the change list.

#### Scenario: Periodic auto-refresh

- **WHEN** TUI is displayed
- **THEN** the change list is refreshed every 5 seconds
- **AND** progress status is updated

#### Scenario: Display continues during refresh

- **WHEN** auto-refresh is in progress
- **THEN** TUI display is not interrupted
- **AND** changes are reflected after refresh completes

### Requirement: New Change Detection

When auto-refresh detects new changes, they SHALL be displayed appropriately.

#### Scenario: New change detection
- **WHEN** auto-refresh detects a new change
- **THEN** the new change is added to the change list
- **AND** a "NEW" badge is displayed
- **AND** "Discovered new change: <id>" is logged

#### Scenario: Default state of new changes
- **WHEN** a new change is detected
- **THEN** it is unselected by default (`[ ]`)
- **AND** the new count in the footer is updated

#### Scenario: NEW badge display
- **WHEN** a change is newly detected
- **THEN** a "NEW" badge is displayed next to the change name
- **AND** the badge is displayed in a visually prominent color

#### Scenario: NEW badge cleared on selection
- **WHEN** user toggles selection on a change with NEW badge in Select mode
- **THEN** the NEW badge is removed
- **AND** the new count in the footer is decremented

#### Scenario: NEW badge cleared on queue addition
- **WHEN** user adds a change with NEW badge to the queue (Running/Stopped mode)
- **THEN** the NEW badge is removed
- **AND** the new count in the footer is decremented

### Requirement: Dynamic Execution Queue
Running 中に queued change を外した場合、当該 change がまだ Processing を開始していないなら、オーケストレータはその change を実行対象から除外しなければならない（MUST）。
Applying/Accepting/Archiving/Resolving の change は `Space` による単体停止要求のみ許可し、`@` は状態変更を行わない（MUST NOT）。

#### Scenario: Running 中に queued change を外す
- **WHEN** TUI が Running モードである
- **AND** ユーザーが queued change を Space キーで NotQueued に切り替える
- **AND** その change が Processing を開始していない
- **THEN** その change は実行対象から除外される
- **AND** 以降の実行でその change は処理されない

#### Scenario: Running 中に実行中 change を単体停止する
- **GIVEN** TUI が Running モードである
- **AND** change の queue_status が Applying/Accepting/Archiving/Resolving のいずれかである
- **WHEN** ユーザーが Space キーを押す
- **THEN** 当該 change の停止要求が発行される
- **AND** 停止完了後に当該 change は `not queued` に戻り、実行マークが解除される
- **AND** 他の queued change は継続して処理される

#### Scenario: Processing 中の change で @ は無効
- **GIVEN** change の queue_status が Applying/Accepting/Archiving/Resolving のいずれかである
- **WHEN** ユーザーが `@` キーを押す
- **THEN** queue_status と選択状態は変更されない

### Requirement: Error State Display

When an error occurs, TUI SHALL explicitly display the error state.

#### Scenario: Mode transition on error

- **WHEN** opencode execution fails with an error (LLM error, insufficient credits, etc.)
- **THEN** TUI mode transitions to "Error"
- **AND** header status displays "Error" in red

#### Scenario: Status panel error display

- **WHEN** TUI is in error state
- **THEN** status panel displays "Error in <change_id>"
- **AND** "Press F5 to retry" guidance is displayed

#### Scenario: Change display in error state

- **WHEN** TUI is in error state
- **THEN** the errored Change status shows "[error]" in red
- **AND** other queued Changes maintain their state

### Requirement: Error Retry with F5 Key

In error state, pressing F5 key SHALL retry processing of the failed Change.

#### Scenario: Retry with F5 key

- **WHEN** TUI is in error state
- **AND** user presses F5 key
- **THEN** the errored Change is added back to the queue
- **AND** TUI transitions to "Running" mode
- **AND** processing resumes

#### Scenario: Log display on retry

- **WHEN** user initiates retry with F5 key
- **THEN** log panel displays "Retrying: <change_id>"

#### Scenario: State after successful retry

- **WHEN** the retried processing succeeds
- **THEN** the Change status updates to "completed" or "archived"
- **AND** remaining queued Changes continue processing

### Requirement: init Subcommand

`init` subcommand SHALL generate a `.cflx.jsonc` configuration template file in the current directory.

#### Scenario: Generate default template (claude)

- **WHEN** user runs `cflx init`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Claude Code template
- **AND** the template includes apply_command, archive_command, analyze_command, and hooks

#### Scenario: Generate opencode template

- **WHEN** user runs `cflx init --template opencode`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with OpenCode template
- **AND** commands use `opencode run` pattern

#### Scenario: Generate claude template explicitly

- **WHEN** user runs `cflx init --template claude`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Claude Code template
- **AND** commands use `claude --dangerously-skip-permissions -p` pattern

#### Scenario: Generate codex template

- **WHEN** user runs `cflx init --template codex`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Codex template
- **AND** commands use `codex` pattern

#### Scenario: Config file already exists without force flag

- **WHEN** user runs `cflx init`
- **AND** `.cflx.jsonc` already exists in the current directory
- **THEN** the command exits with an error
- **AND** an error message indicates the file already exists
- **AND** suggests using `--force` to overwrite

#### Scenario: Overwrite existing config with force flag

- **WHEN** user runs `cflx init --force`
- **AND** `.cflx.jsonc` already exists in the current directory
- **THEN** the existing file is overwritten with the new template
- **AND** a success message is displayed

#### Scenario: Invalid template name

- **WHEN** user runs `cflx init --template invalid`
- **THEN** the command exits with an error
- **AND** an error message lists valid template options (opencode, claude, codex)

### Requirement: Footer Dynamic Guidance Display

The selection mode footer SHALL display appropriate guidance messages based on application state.

#### Scenario: Guidance when no changes

- **WHEN** TUI is in selection mode
- **AND** the change list is empty
- **THEN** the footer displays "Add new proposals to get started"

#### Scenario: Guidance when no changes selected

- **WHEN** TUI is in selection mode
- **AND** one or more changes exist
- **AND** no changes are selected
- **THEN** the footer displays "Select changes with Space to process"

#### Scenario: Guidance when changes are selected

- **WHEN** TUI is in selection mode
- **AND** one or more changes are selected
- **THEN** the footer displays "Press F5/! to start processing" when no TUI config override exists

### Requirement: Running Footer Progress Bar Display

The running mode footer SHALL display a progress bar for overall processing progress.

#### Scenario: Completed task progress retention
- **WHEN** a change transitions to Completed, Archived, or Merged state
- **AND** the latest tasks.md is available at apply completion or archive start
- **THEN** the TUI preserves the last known task counts for that change
- **AND** the progress percentage does not decrease (monotonically increasing)

### Requirement: Processing Item Spinner Animation

The TUI SHALL display phase-specific vocabulary for active work: `preparing`, `applying`, `accepting`, `rejecting`, `archiving`, and `resolving`. `preparing` SHALL mean that a scheduler-admitted change is creating, recreating, setting up, inspecting, or waiting to start its repository-derived workflow operation after acquiring an execution slot. It SHALL display `blocked` for both dependency waits and validated external prerequisite waits, with detail that identifies the blocker kind. It SHALL display `stalled` for no-progress or exhausted execution holds. When an iteration number applies, the display SHALL retain the `status:iteration` format. TUI, WebUI, and `/api/v2` SHALL project the same reducer-owned active status.

#### Scenario: Dependency wait displays blocked

- **GIVEN** a change waits on an unarchived proposal dependency
- **WHEN** the TUI renders the change
- **THEN** its status is `blocked`
- **AND** its detail identifies a dependency blocker

#### Scenario: External prerequisite displays blocked

- **GIVEN** the orchestrator has validated an external prerequisite blocker
- **WHEN** the TUI renders the change
- **THEN** its status is `blocked`
- **AND** its detail exposes the external category, unblock condition, and next action

#### Scenario: Exhausted execution displays stalled

- **GIVEN** automatic execution stopped after no progress or retry exhaustion
- **WHEN** the TUI renders the change
- **THEN** its status is `stalled`
- **AND** the row is not described as waiting on a dependency or external prerequisite

#### Scenario: Worktree setup displays preparing

- **GIVEN** a queued change has acquired a parallel execution-slot permit and passed stop and terminal gates
- **WHEN** the scheduler begins force-recreate cleanup, managed worktree creation, or `.wt/setup`
- **THEN** the shared status becomes `preparing` before the potentially slow preparation starts
- **AND** TUI, WebUI, and `/api/v2` display `preparing` while preparation remains in progress
- **AND** the status does not claim that Apply has started

#### Scenario: Preparation advances to the repository-derived phase

- **GIVEN** a change is displayed as `preparing`
- **WHEN** workspace preparation completes and repository evidence selects the next workflow operation
- **THEN** the status changes to that operation's active vocabulary
- **AND** an Apply route displays `applying` with its applicable iteration
- **AND** a resumed acceptance, rejection, archive, or resolve route does not emit a false Apply transition

#### Scenario: Preparation failure is visible

- **GIVEN** a change is displayed as `preparing`
- **WHEN** worktree creation or `.wt/setup` fails
- **THEN** the change transitions to `error`
- **AND** the operator receives a diagnostic identifying the failed preparation step

#### Scenario: Preparing is active for safety controls

- **GIVEN** a change is displayed as `preparing`
- **WHEN** an operator requests dequeue or managed-worktree deletion
- **THEN** the system treats the change as active execution
- **AND** managed-worktree deletion remains refused
- **AND** if inline preparation has no termination handle, immediate dequeue is refused while the stop mark remains recorded
- **AND** after preparation returns, the recorded stop prevents operation-agent startup and the change leaves `preparing` through a reducer-visible stopped or cleared transition

#### Scenario: Preparing clears on pre-operation exit

- **GIVEN** a change has emitted `preparing`
- **WHEN** global cancellation or a pre-spawn early return ends dispatch before another operation-started event
- **THEN** the reducer receives a clearing, stopped, or terminal transition
- **AND** the change does not remain indefinitely displayed as `preparing`

#### Scenario: Preparing is not durable routing state

- **GIVEN** a process stops while a change is displayed as `preparing`
- **WHEN** Conflux starts again with the same workspace files and Git state
- **THEN** the next action is derived from workspace and repository evidence
- **AND** no persisted `preparing` observation, log, metric, or elapsed duration controls routing

#### Scenario: Setup duration is observable

- **GIVEN** a managed worktree contains `.wt/setup`
- **WHEN** Conflux runs the setup script
- **THEN** it emits one setup-start diagnostic
- **AND** success emits one completion diagnostic with elapsed duration
- **AND** failure emits one actionable failure diagnostic
- **AND** these diagnostics do not change workflow routing

### Requirement: Completion Detection Retry Settings

Retry behavior SHALL be implemented for completion state detection.

#### Scenario: Default retry settings

- **WHEN** no retry settings exist in configuration file
- **THEN** maximum retry count is 3
- **AND** retry interval is 500 milliseconds

#### Scenario: Cancel during retry

- **WHEN** retry loop is in progress
- **AND** cancellation token is cancelled
- **THEN** retry loop terminates immediately
- **AND** process is cleaned up properly

### Requirement: TUI Unicode Display Width Support

The TUI SHALL correctly calculate and truncate text based on Unicode display width, not byte length or character count.

#### Scenario: Japanese text truncation in logs
- **WHEN** a log message contains Japanese characters (e.g., "設定ファイル初期化")
- **AND** the message exceeds the available display width
- **THEN** the message is truncated at a valid display width boundary
- **AND** ellipsis "..." is appended
- **AND** no panic occurs due to UTF-8 boundary issues

#### Scenario: Mixed ASCII and CJK text
- **WHEN** a log message contains both ASCII and CJK characters
- **THEN** ASCII characters count as 1 display column
- **AND** CJK characters count as 2 display columns
- **AND** truncation respects the total display width

#### Scenario: Emoji handling
- **WHEN** a log message contains emoji characters
- **THEN** emoji characters are counted with their proper display width
- **AND** truncation does not split emoji sequences

#### Scenario: ANSI control codes are stripped before display
- **WHEN** a log message contains ANSI control sequences (e.g., "\x1b[96mRead\x1b[0m")
- **THEN** the log panel displays the message without those control sequences (e.g., "Read")
- **AND** display width calculation and truncation operate on the stripped text

### Requirement: Native Task Progress Parsing

The system SHALL provide native change list discovery by directly reading the filesystem instead of relying on external commands.

#### Scenario: List all changes natively

```
Given openspec/changes directory exists
And it contains subdirectories ["change-a", "change-b"]
When list_changes_native() is called
Then it returns Vec<Change> with 2 entries
And each Change has id matching directory name
And each Change has task counts from tasks.md
```

#### Scenario: Handle missing tasks.md gracefully

```
Given openspec/changes/my-change directory exists
And tasks.md file does not exist in that directory
When list_changes_native() is called
Then the change is included with completed_tasks=0 and total_tasks=0
```

#### Scenario: Empty changes directory

```
Given openspec/changes directory exists but is empty
When list_changes_native() is called
Then it returns empty Vec<Change>
```

#### Scenario: Changes directory does not exist

```
Given openspec/changes directory does not exist
When list_changes_native() is called
Then it returns empty Vec<Change>
```

### Requirement: Task Progress Fallback Behavior

The system SHALL use native task parsing as primary source when openspec CLI returns zero task counts.

#### Scenario: CLI returns zero tasks
- **WHEN** openspec CLI returns `completedTasks: 0, totalTasks: 0` for a change
- **AND** a `tasks.md` file exists for that change
- **THEN** the system uses native parsing to determine actual task counts
- **AND** the TUI displays the native-parsed task counts

#### Scenario: CLI returns non-zero tasks
- **WHEN** openspec CLI returns non-zero task counts for a change
- **THEN** the system uses the CLI-provided task counts
- **AND** native parsing is not performed for that change

### Requirement: Version Display

The CLI SHALL support a `--version` flag to display the application version with UTC build number.

#### Scenario: Display version with --version flag
- **WHEN** user runs `cflx --version`
- **THEN** the application version is displayed in `v<semver>(YYYYMMDDHHmmss)` format
- **AND** the build number uses UTC time
- **AND** the program exits with code 0

#### Scenario: Display version with -V short flag
- **WHEN** user runs `cflx -V`
- **THEN** the application version is displayed in `v<semver>(YYYYMMDDHHmmss)` format

### Requirement: TUI Header Version Display

The TUI header SHALL display the application version with UTC build number in both selection and running modes.

#### Scenario: Version in selection mode header
- **WHEN** TUI is in selection mode
- **THEN** the header displays the application version (e.g., "v0.1.0(20260117113311)")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

#### Scenario: Version in running mode header
- **WHEN** TUI is in running mode
- **THEN** the header displays the application version (e.g., "v0.1.0(20260117113311)")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

### Requirement: Terminal Status Task Count Display

TUI は反復回数がある状態の表示を `status:iteration` 形式にしなければならない（SHALL）。apply/acceptance/archive/resolve の iteration 番号が更新された場合、Changes 一覧のステータス表示は最新の iteration に同期し続けなければならない（SHALL）。Applying中のChanges行では、ステータスは`[status:iteration]`のみを表示し、タスク進捗は`<completed>/<total>(<percent>%)`形式で表示しなければならない（SHALL）。

#### Scenario: Applying の iteration 表示
- **GIVEN** change が apply 実行中である
- **AND** apply の iteration 番号が 1 である
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `applying:1` となる

#### Scenario: Archiving の iteration 表示
- **GIVEN** change が archive 実行中である
- **AND** archive の iteration 番号が 2 である
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `archiving:2` となる

#### Scenario: Applying の iteration 更新に追従する
- **GIVEN** change の queue_status が applying である
- **AND** iteration_number が 2 から 3 に更新される
- **WHEN** TUI が Changes 一覧を再描画する
- **THEN** ステータス表示は `applying:3` となる

#### Scenario: Applying の進捗表示フォーマット
- **GIVEN** change の queue_status が applying である
- **AND** iteration 番号が 1 である
- **AND** completed_tasks が 0 で total_tasks が 3 である
- **WHEN** TUI が change 行を表示する
- **THEN** Changes 行の進捗表示は `0/3(0%)` となる

### Requirement: TUI Archive Priority Processing

The TUI running mode SHALL archive all completed changes before starting the next apply operation.

#### Scenario: Archive before next apply
- **WHEN** TUI is in running mode
- **AND** one or more queued changes have reached 100% task completion
- **THEN** all complete changes are archived before any new apply command starts
- **AND** the archive process follows the same hooks (pre_archive, post_archive) as normal archiving

#### Scenario: Multiple complete changes
- **WHEN** TUI is in running mode
- **AND** multiple changes reach 100% completion simultaneously
- **THEN** all complete changes are archived in sequence
- **AND** processing continues only after all complete changes are archived

#### Scenario: Archive on loop iteration
- **WHEN** TUI orchestrator starts a new processing iteration
- **THEN** it first checks for any complete changes in the queue
- **AND** archives all complete changes before selecting the next change to apply

### Requirement: Remove Retry-Based Completion Check

The TUI SHALL NOT rely on retry loops to detect task completion for archiving purposes.

#### Scenario: Immediate archive attempt after apply success
- **WHEN** an apply command completes successfully
- **THEN** the orchestrator immediately returns to the main loop
- **AND** the main loop's archive phase handles completion detection
- **AND** no arbitrary retry delays are used for completion detection

#### Scenario: Completion detected on next iteration
- **WHEN** a change becomes 100% complete during another change's apply
- **THEN** the complete change is detected and archived on the next loop iteration
- **AND** no warning about "did not reach completion state" is logged

### Requirement: Reliable Archive Tracking

archive 検証は `openspec/changes/{change_id}` が存在する場合に未アーカイブとして扱わなければならない（SHALL）。

archive 検証と archived change 解決は、archive entry として direct match (`openspec/changes/archive/<change_id>`) と date-prefixed match (`openspec/changes/archive/<date>-<change_id>`) の両方を同一 change として扱わなければならない（MUST）。

archive 検証と archived change 解決は、nested date directory layout (`openspec/changes/archive/<date>/<change_id>`) を valid archive entry として扱ってはならない（MUST NOT）。

archive 検証は active change directory が存在しない場合でも、valid archive entry が存在しない、または matching invalid archive layout が存在するなら archive complete として扱ってはならない（MUST NOT）。

invalid archive layout の診断は、offending path と expected `openspec/changes/archive/YYYY-MM-DD-<change_id>` layout を含まなければならない（MUST）。

#### Scenario: changes が残っている場合は未アーカイブ扱い
- **WHEN** archive コマンドが成功する
- **AND** `openspec/changes/{change_id}` が存在している
- **THEN** archive 検証は未アーカイブとして扱われる
- **AND** archive コマンドは再実行される

#### Scenario: dated archive entry is treated as archived completion
- **GIVEN** `openspec/changes/{change_id}` は存在しない
- **AND** `openspec/changes/archive/2026-01-08-{change_id}` が存在する
- **WHEN** archive completion verification or archived change resolution runs for `{change_id}`
- **THEN** the change is treated as archived
- **AND** the implementation does not require a direct-match archive directory to exist

#### Scenario: nested archive layout is rejected
- **GIVEN** `openspec/changes/{change_id}` does not exist
- **AND** `openspec/changes/archive/2026-07-09/{change_id}/tasks.md` exists
- **WHEN** archive completion verification or archived change resolution runs for `{change_id}`
- **THEN** the change is not treated as archived
- **AND** the result reports invalid archive layout
- **AND** the diagnostic includes `openspec/changes/archive/2026-07-09/{change_id}`
- **AND** the diagnostic identifies `openspec/changes/archive/YYYY-MM-DD-{change_id}` as the expected layout

### Requirement: TUI Uses Native Change Discovery

The TUI mode MUST use native directory scanning instead of external `openspec list` command for all change list operations.

#### Scenario: Initial change list uses native implementation

```
Given TUI mode is started
When initial changes are loaded
Then openspec/changes directory is read directly
And no external openspec command is executed for listing
```

#### Scenario: Auto-refresh uses native implementation

```
Given TUI is in running mode
When auto-refresh interval triggers
Then openspec/changes directory is read directly
And no external openspec command is executed for listing
```

#### Scenario: Archive phase uses native implementation

```
Given TUI orchestrator is processing changes
When checking for complete changes to archive
Then openspec/changes directory is read directly
And task progress is determined from tasks.md files
```

### Requirement: Log Panel Scroll Feature

The TUI log panel SHALL support scrolling to view older log entries.

#### Scenario: Page Down scroll in log panel
- **WHEN** TUI is in running mode
- **AND** log entries exceed visible area
- **AND** user presses Page Down key
- **THEN** log view scrolls down by one page
- **AND** scroll position is limited to show the most recent entries at the bottom

#### Scenario: Page Up scroll in log panel
- **WHEN** TUI is in running mode
- **AND** log entries exceed visible area
- **AND** user presses Page Up key
- **THEN** log view scrolls up by one page
- **AND** scroll position stops at the oldest log entry

#### Scenario: Scroll position indicator display
- **WHEN** log entries exceed visible area
- **THEN** the log panel title displays current scroll position (e.g., "Logs [1-10/50]")
- **AND** the indicator shows visible range and total count

#### Scenario: Auto-scroll on new log entry
- **WHEN** a new log entry is added
- **AND** user has not scrolled up manually (auto_scroll is true)
- **THEN** log view automatically scrolls to show the newest entry

#### Scenario: Disable auto-scroll when scrolling up
- **WHEN** user scrolls up in log panel (Page Up)
- **THEN** auto-scroll is disabled
- **AND** new log entries do not change scroll position
- **AND** user can review historical logs without interruption

#### Scenario: Re-enable auto-scroll at bottom
- **WHEN** user scrolls down to the bottom of logs
- **THEN** auto-scroll is re-enabled
- **AND** subsequent new entries will auto-scroll into view

#### Scenario: Home key jump to oldest log
- **WHEN** TUI is in running mode
- **AND** log entries exist
- **AND** user presses Home key
- **THEN** log view jumps to the oldest log entry
- **AND** auto-scroll is disabled

#### Scenario: End key jump to newest log
- **WHEN** TUI is in running mode
- **AND** log entries exist
- **AND** user presses End key
- **THEN** log view jumps to the newest log entry
- **AND** auto-scroll is re-enabled

#### Scenario: Mouse wheel scroll up
- **WHEN** TUI is in running mode
- **AND** log entries exceed visible area
- **AND** user scrolls mouse wheel up
- **THEN** log view scrolls up by a few lines (e.g., 3 lines)
- **AND** auto-scroll is disabled

#### Scenario: Mouse wheel scroll down
- **WHEN** TUI is in running mode
- **AND** log entries exceed visible area
- **AND** user scrolls mouse wheel down
- **THEN** log view scrolls down by a few lines (e.g., 3 lines)
- **AND** if scroll position reaches the bottom, auto-scroll is re-enabled

#### Scenario: Analysis log header includes iteration
- **WHEN** the TUI receives analysis output logs with an iteration number
- **THEN** the log entry header is displayed as `[analysis:<iteration>]`
- **AND** the header is rendered even when no change_id is present

#### Scenario: Analysis log header without iteration
- **WHEN** the TUI receives analysis output logs without an iteration number
- **THEN** the log entry header is displayed as `[analysis]`

#### Scenario: Resolve log header includes change id and iteration
- **WHEN** the TUI receives resolve output logs with change_id and iteration
- **THEN** the log entry header is displayed as `[{change_id}:resolve:<iteration>]`

#### Scenario: Resolve log header without iteration
- **WHEN** the TUI receives resolve output logs with change_id but no iteration
- **THEN** the log entry header is displayed as `[{change_id}:resolve]`

### Requirement: approve Subcommand

The CLI SHALL NOT provide an `approve` subcommand.

#### Scenario: Approve subcommand is rejected
- **WHEN** user runs `cflx approve set {change_id}`
- **THEN** CLI reports an unknown subcommand error
- **AND** exit code is non-zero

### Requirement: TUI Approval Toggle

The TUI SHALL ignore approval toggles and SHALL NOT change any state on `@` key presses.

#### Scenario: @ key does nothing
- **WHEN** user presses `@` key in any TUI mode
- **THEN** selection and queue status are unchanged
- **AND** no approval state is created or stored

### Requirement: Auto-Queue Approved Changes on TUI Startup

The TUI SHALL start with all changes unselected and SHALL NOT auto-queue any change. Active-change refresh and preserved worktree discovery MAY populate display/catalog state, but MUST NOT create execution eligibility. Only marked IDs accepted by Start or later accepted shared operator queue/retry intent may enter ordinary execution.

#### Scenario: TUI startup clears execution marks

**When**: The user starts the TUI
**Then**: All changes are unselected by default
**And**: No changes are automatically queued or admitted to execution

#### Scenario: Initial all-change refresh preserves selection boundary

**Given**: `fresh` is marked and `stale` is unmarked
**And**: `stale` has a preserved recoverable worktree
**When**: The user starts processing `fresh`
**And**: The initial `ChangesRefreshed` event contains both changes
**Then**: Only `fresh` enters ordinary execution eligibility
**And**: Catalog registration of `stale` does not queue, analyze, or execute it

#### Scenario: Explicit later queue enables preserved workspace recovery

**Given**: `stale` remains visible and unqueued with a preserved recoverable worktree
**When**: The user explicitly adds `stale` to the Running-mode queue
**Then**: Shared reducer queue intent makes `stale` eligible
**And**: Conflux derives its resume phase from workspace and Git evidence

#### Scenario: Queue removal revokes recovery eligibility

**Given**: `stale` was explicitly queued and has not yet completed
**When**: The user removes or successfully stops and dequeues `stale`
**Then**: Preserved worktree discovery does not requeue it
**And**: Explicit requeue is required before it can execute again

### Requirement: Unapproved Changes Cannot Be Queued

The system SHALL allow changes to be queued regardless of approval state.

#### Scenario: TUI can queue any change
- **WHEN** TUI is in selection mode
- **AND** user presses Space to select a change
- **THEN** the change is queued without approval checks

#### Scenario: CLI run includes specified change
- **WHEN** user runs `cflx run --change {change_id}`
- **THEN** the change is added to the queue
- **AND** no approval warning is displayed

### Requirement: Log Entry Limit

The TUI SHALL maintain a maximum limit on stored log entries to prevent unbounded memory growth.

#### Scenario: Log entry limit enforcement
- **WHEN** a new log entry is added
- **AND** the total log count exceeds 1000 entries
- **THEN** the oldest log entry is removed
- **AND** scroll offset is adjusted if necessary to prevent display issues

### Requirement: Log Entry Headers

TUI は analysis と resolve の operation ログに対して構造化ヘッダを表示し、追跡性を向上させなければならない (SHALL)。

#### Scenario: Analysis ログヘッダ形式
- **WHEN** analysis operation がログメッセージを出力する
- **THEN** ログエントリは `[analysis:N]` のヘッダで表示される
- **AND** N は analysis operation の iteration number を表す

#### Scenario: Resolve ログヘッダ形式
- **WHEN** resolve operation がログメッセージを出力する
- **THEN** ログエントリは `[resolve:N]` のヘッダで表示される
- **AND** N は resolve operation の iteration number を表す
- **AND** ヘッダには change_id が表示されない

#### Scenario: ログヘッダのカラーリングは一貫している
- **WHEN** ヘッダ付きログエントリが表示される
- **THEN** change_id が利用可能な場合、ヘッダは change_id hash に基づいた色分けで表示される
- **AND** 視認性のためヘッダは太字で表示される

### Requirement: TUI Status Transition on Apply Completion

The TUI SHALL transition change status from `Processing` to `Completed` when an apply operation succeeds and all tasks are complete.

#### Scenario: Apply succeeds with 100% task completion

- **GIVEN** a change is being processed in running mode
- **AND** the change has `Processing` status
- **WHEN** the apply command completes successfully
- **AND** all tasks for the change are marked complete (100%)
- **THEN** the change status transitions to `Completed`
- **AND** the status display shows `[completed]` instead of spinner
- **AND** a log entry "Completed: <change-id>" is added

#### Scenario: Apply succeeds with incomplete tasks

- **GIVEN** a change is being processed in running mode
- **AND** the change has `Processing` status
- **WHEN** the apply command completes successfully
- **AND** some tasks remain incomplete (< 100%)
- **THEN** the change status remains `Processing`
- **AND** the orchestrator continues to next apply iteration

#### Scenario: 100% complete change displays correctly before archive

- **GIVEN** a change has completed all tasks (100%)
- **AND** the change has `Completed` status
- **WHEN** the TUI renders the change list
- **THEN** the status shows `[completed]` (not `Processing...` with 100%)
- **AND** the progress column shows the task count (e.g., `29/29`)

### Requirement: Archive Phase Does Not Reset Status

The TUI archive phase SHALL NOT send redundant status transition events for changes that are already in `Completed` status.

#### Scenario: Archive already-completed change

- **GIVEN** a change has `Completed` status
- **WHEN** the archive phase processes the change
- **THEN** no `ProcessingStarted` event is sent
- **AND** no additional `ProcessingCompleted` event is sent
- **AND** only `ChangeArchived` event is sent upon successful archive

#### Scenario: Archive pre-complete change from queue

- **GIVEN** a change was 100% complete before being queued
- **AND** the change has `Queued` status (not yet processed)
- **WHEN** the archive phase identifies the change as complete
- **THEN** `ProcessingStarted` event is sent (status → Processing)
- **AND** `ProcessingCompleted` event is sent (status → Completed)
- **AND** archive command is executed
- **AND** `ChangeArchived` event is sent (status → Archived)

### Requirement: Apply Context History

The orchestrator MUST use one history-injection loop for every managed-worktree apply attempt. Each attempt summary and one-shot acceptance failure tail MUST retain their existing bounded injection semantics.

#### Scenario: second apply includes history

- **GIVEN** a change's first managed-worktree apply returned an agent summary
- **WHEN** its second apply starts
- **THEN** the prompt includes `<last_apply attempt="1">`
- **AND** the block contains the first summary

### Requirement: Apply History Context Format

apply 履歴コンテキストは、archive と resolve の履歴フォーマットと一貫性を持たなければならない（SHALL）。

#### Scenario: 履歴フォーマットの一貫性

- **GIVEN** システムが apply、archive、resolve の履歴を管理する
- **WHEN** 各履歴がフォーマットされる
- **THEN** すべての履歴で XML 風のタグ形式が使用される
- **AND** すべての履歴で `attempt`、`status`、`duration` フィールドが含まれる
- **AND** 各操作固有の追加情報（`error`、`verification_result`、`continuation_reason`）も含まれる

### Requirement: TUI Stop Processing with Escape Key

TUIはEsc二度押しによる停止時、現在の実行活動を確認しなければならない（SHALL）。現在のエージェントプロセスまたはin-flight実行が存在する場合は、そのプロセスと子プロセスを確実に終了しなければならない（SHALL）。実行活動が存在せずparallel schedulerが待機しているだけの場合は、scheduler/orchestratorを停止しなければならず（SHALL）、プロセスを強制終了したと表示してはならない（MUST NOT）。進行中のbackground mergeまたはbase-lane mutationは安全な停止境界まで完了を待たなければならないが（SHALL）、それ自体をエージェントプロセスのforce stopと表示してはならない（MUST NOT）。

#### Scenario: 強制停止で子プロセスが残らない

- **GIVEN** 現在のエージェントプロセスまたはin-flight実行が存在する
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** 現在のエージェントプロセスとその子プロセスが終了する
- **AND** 終了待機がタイムアウトした場合でも、追加の終了処理が行われる
- **AND** ログは実際のforce stopを表示する
- **AND** 変更の状態はNotQueuedに戻る
- **AND** 実行マークは保持される

#### Scenario: 実行プロセスがない待機状態を通常停止する

- **GIVEN** parallel orchestratorは動作中である
- **AND** 対象changeは`MergeWait`、`ResolveWait`、deferred merge、またはscheduler idleで待機している
- **AND** 現在のエージェントプロセスおよびin-flight実行は存在しない
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** scheduler/orchestratorは停止する
- **AND** ログは`Processing stopped`を一度だけ表示する
- **AND** `Force stopped`またはプロセス終了を主張するログを表示しない
- **AND** 存在しないプロセスへの終了要求を行わない
- **AND** 遅延した停止イベントが到着しても`Processing stopped`を重複表示しない
- **AND** 変更の状態はNotQueuedに戻る
- **AND** 実行マークは保持される

#### Scenario: 進行中background mergeを安全に停止する

- **GIVEN** parallel orchestratorは進行中のbackground mergeまたはbase-lane mutationを所有している
- **AND** 現在のエージェントプロセスおよびin-flight agent executionは存在しない
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** operator cancellationが要求される
- **AND** terminal stopはmergeまたはbase-lane operationが既存の安全な結果境界へ到達するまで待つ
- **AND** ログは`Force stopped`またはエージェントプロセス終了を主張しない
- **AND** cancellation待機が有界期限へ到達してもexecution failureとは分類しない

### Requirement: TUI Stopped Mode

The TUI SHALL provide an internal Stopped mode that manages change state by holding queued status only during execution. When transitioning to Stopped, queue_status SHALL be reset to NotQueued while preserving execution marks ([x]). Space operations in Stopped mode SHALL only add/remove execution marks while maintaining queue_status as NotQueued. When resuming with F5, execution-marked changes SHALL be restored to queued and processing SHALL resume. Task progress updates in Stopped mode SHALL NOT trigger queuing. The header SHALL project this inactive resumable mode as `Ready`; mode-specific controls SHALL continue to identify F5 as `resume`.

#### Scenario: Stopped mode display
- **WHEN** TUI is in Stopped mode
- **THEN** header status displays "Ready" in cyan color
- **AND** status controls display the configured start key as `resume`
- **AND** the change list remains visible with current statuses
- **AND** execution-marked changes show "[x]" while their queue_status remains not queued
- **AND** the internal execution mode remains Stopped

#### Scenario: Queue management in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on an execution-marked change
- **THEN** the execution mark is removed and queue_status remains not queued

#### Scenario: Queue addition in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-marked change
- **THEN** the execution mark is added and queue_status remains not queued

#### Scenario: Task completion in Stopped mode does not auto-queue
- **WHEN** TUI is in Stopped mode
- **AND** a change's tasks are updated (e.g., all tasks marked complete)
- **THEN** the change queue_status SHALL remain not queued
- **AND** the change SHALL NOT be automatically added to the queue

#### Scenario: Resume processing from Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** one or more changes are execution-marked
- **AND** user presses F5
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes after converting execution-marked changes to queued
- **AND** log displays "Resuming processing..."

#### Scenario: Resume with empty queue shows warning
- **WHEN** TUI is in Stopped mode
- **AND** no changes are execution-marked
- **AND** user presses F5
- **THEN** a warning message is displayed
- **AND** the TUI remains in Stopped mode

### Requirement: TUI Help Text for Stop

The TUI help text SHALL include stop key binding information.

#### Scenario: Stopping mode help text
- **WHEN** TUI is in Stopping mode
- **THEN** help text includes "Esc: force stop"
- **AND** help text includes "F5: continue"
- **AND** help text shows "Waiting for current process..."

### Requirement: Interrupted Change Handling
Changes interrupted by stop SHALL be handled according to the policy of holding queued status only during execution. When force-stopped, queue_status SHALL be reset to NotQueued while preserving execution marks. On resume, execution-marked changes SHALL be restored to queued and can be re-processed. Accepting status SHALL be treated as an in-flight execution state and MUST be reset to NotQueued when the user force-stops.

#### Scenario: Force-stopped accepting change returns to not queued
- **GIVEN** a change is in Accepting status
- **WHEN** the user force stops with second Esc press
- **THEN** the change status becomes not queued
- **AND** the execution mark remains set

### Requirement: Parallel Execution Mode Flag

The CLI SHALL use cumulative Git-worktree orchestration for every executable `run` invocation. Execution mode SHALL NOT be selectable through a `--parallel` or serial-equivalent flag.

#### Scenario: Default run uses worktree orchestration

- **WHEN** user runs `cflx run --all` in a usable Git repository
- **THEN** the orchestrator analyzes eligible changes and dispatches cumulative worktree execution
- **AND** no execution-mode flag is required

#### Scenario: Single change uses the same execution path

- **WHEN** user runs `cflx run my-feature` in a usable Git repository
- **THEN** `my-feature` executes in a managed worktree
- **AND** it follows the same archive and post-archive state transitions as a multi-change run

#### Scenario: Retired parallel flag is rejected

- **WHEN** user runs `cflx run --parallel`
- **THEN** argument parsing fails with a non-zero exit status
- **AND** help output does not advertise `--parallel`

#### Scenario: Execution requires Git before side effects

- **WHEN** user starts executable run orchestration outside a usable Git repository or without the Git command
- **THEN** startup fails with a non-zero exit status and an actionable error
- **AND** no hook, lifecycle adapter, AI subprocess, or managed-worktree mutation has started

#### Scenario: Concurrency remains configurable

- **WHEN** user runs `cflx run --all --max-concurrent 4`
- **THEN** at most 4 managed workspaces execute simultaneously
- **AND** additional eligible changes wait until capacity is available

### Requirement: Parallel Mode TUI Display

The TUI SHALL display worktree execution, workspace, and post-archive progress as the standard execution presentation. It SHALL NOT display a selectable execution-mode badge or mode toggle.

#### Scenario: Display concurrent worktree progress

- **WHEN** TUI is running one or more changes
- **THEN** each change shows its managed-workspace status
- **AND** concurrently active changes may show simultaneous progress

#### Scenario: Display post-archive progress

- **WHEN** an archived change enters merge, resolve, or push handling
- **THEN** the corresponding progress and terminal result are displayed
- **AND** archive alone is not presented as terminal because of an execution mode

### Requirement: Parallel Mode Dry Run

The CLI SHALL support `--dry-run` to preview dependency groups without execution on the standard worktree orchestration path.

#### Scenario: Preview dependency groups

- **WHEN** user runs `cflx run --all --dry-run`
- **THEN** the analyzer determines execution groups
- **AND** the groups are displayed without executing changes
- **AND** no managed workspace is created or mutated

### Requirement: VCS Backend Selection Flag

CLI SHALL apply `--vcs` directly to the sole worktree execution path.

#### Scenario: Default auto detection

- **WHEN** an executable run starts without `--vcs`
- **THEN** the VCS backend is auto-detected
- **AND** Git is selected in a usable Git repository

#### Scenario: Explicit git selection

- **WHEN** `cflx run --all --vcs git` is executed
- **THEN** the Git backend is used
- **AND** startup fails before orchestration side effects if Git is unavailable

#### Scenario: Explicit auto selection

- **WHEN** `cflx run --all --vcs auto` is executed
- **THEN** the VCS backend is auto-detected
- **AND** Git is selected in a usable Git repository

#### Scenario: Invalid VCS value

- **WHEN** `cflx run --all --vcs invalid` is executed
- **THEN** error message "Invalid VCS backend: invalid. Valid options: auto, git" is displayed
- **AND** exit code is non-zero

### Requirement: Git Uncommitted Changes Error Message

Git backend で未コミット変更がある場合、CLI は詳細なエラーメッセージを表示しなければならない（SHALL）。
未追跡ファイルの判定では `.gitignore` と `.git/info/exclude` の除外を適用しなければならない（MUST）。

#### Scenario: Error message format
- **WHEN** parallel execution is attempted with Git backend
- **AND** uncommitted changes exist
- **THEN** the error message includes:
  - Problem description
  - Resolution method (commit or stash)
  - Specific command examples

#### Scenario: Untracked files also trigger error
- **WHEN** parallel execution is attempted with Git backend
- **AND** only untracked files exist
- **THEN** the same error message is displayed
- **AND** files in `.gitignore` と `.git/info/exclude` は除外される

### Requirement: Archived 状態の checkbox 表示

TUI は terminal row の checkbox / execution mark semantics を、その row が execution candidate かどうかに応じて表現しなければならない（SHALL）。

Archived 状態の change は既存どおり checkbox をグレー表示してよい。一方で rejected 状態の change は execution candidate ではないため、以前の execution mark を保持したまま表示してはならない（MUST NOT）。

#### Scenario: rejected 状態では x マークを保持しない

- **GIVEN** TUI が change 一覧を表示している
- **AND** ある change が rejection flow 完了により `rejected` 状態へ遷移した
- **WHEN** 画面が次にレンダリングされる
- **THEN** その change は execution mark なし (`selected = false`) で表示される
- **AND** ステータス表示は `rejected` のままである

### Requirement: Rejected terminal row の execution mark クリア

TUI は `rejected` terminal row を execution candidate として扱ってはならない（SHALL NOT）。
`ChangeRejected` 遷移を受けた行は `selected=false` へ遷移し、他 change の execution mark は保持しなければならない（SHALL）。

#### Scenario: rejected transition clears only target mark

- **GIVEN** change `foo` と `bar` が execution mark 付きで queued 表示である
- **WHEN** `foo` が `ChangeRejected` で `rejected` に遷移する
- **THEN** `foo` の execution mark は clear される
- **AND** `bar` の execution mark は保持される

#### Scenario: 実行モードで archived 状態の change の checkbox がグレー表示

- **GIVEN** TUI が実行モードである
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change の checkbox 部分は `Color::DarkGray` で表示される
- **AND** checkbox のテキストは `[x]` のまま（内容は変わらない）

#### Scenario: 選択モードに戻った際も archived 状態は維持

- **GIVEN** 処理が完了し TUI が選択モードに戻った
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change の checkbox 部分は `Color::DarkGray` で表示される

### Requirement: Web Monitoring Flags

The CLI SHALL expose the browser-facing `--web` TCP listener and the default local Unix API listener as distinct controls. In web-enabled builds, default TUI, `tui`, and `run` SHALL use `${GIT_COMMON_DIR}/cflx-api.sock` unless `--web-unix-socket PATH` overrides it or `--no-web-unix-socket` disables it. The override and opt-out SHALL be mutually exclusive. `--web` SHALL add the retained TCP/Web UI listener without disabling UDS.

#### Scenario: Default UDS starts without web flag

- **GIVEN** a web-enabled build inside a Git repository
- **WHEN** the user starts default TUI, `cflx tui`, or `cflx run` without Unix socket flags
- **THEN** the API binds `${GIT_COMMON_DIR}/cflx-api.sock`
- **AND** no TCP Web UI listener starts unless `--web` is supplied

#### Scenario: Override default Unix path

- **WHEN** the user supplies `--web-unix-socket /run/user/1000/custom.sock`
- **THEN** the API binds that path instead of the Git common-directory default

#### Scenario: Disable default Unix listener

- **WHEN** the user supplies `--no-web-unix-socket`
- **THEN** no UDS listener starts
- **AND** local orchestration may continue

#### Scenario: Unix options are mutually exclusive

- **WHEN** the user supplies both `--web-unix-socket PATH` and `--no-web-unix-socket`
- **THEN** CLI parsing fails with an actionable conflict error

#### Scenario: Enable web monitoring alongside UDS

- **WHEN** the user runs with `--web`
- **THEN** the retained TCP server starts on the configured bind and actual port
- **AND** the default or explicit UDS remains active
- **AND** the TUI displays and encodes only the TCP Web UI URL as QR

#### Scenario: Configure TCP listener

- **WHEN** the user runs with `--web --web-bind 0.0.0.0 --web-port 3000` and valid required authentication
- **THEN** the TCP server accepts connections on port 3000 from the configured interface
- **AND** the UDS path remains controlled only by its default, override, or opt-out

#### Scenario: Non-Git invocation requires a decision

- **GIVEN** a web-enabled local orchestration invocation outside Git
- **WHEN** neither `--web-unix-socket PATH` nor `--no-web-unix-socket` is supplied
- **THEN** startup exits non-zero before orchestration
- **AND** the error explains both explicit choices

### Requirement: Archived change の一覧保持

TUI は archived 状態になった change をアプリ終了まで Changes 一覧に残さなければならない（SHALL）。

#### Scenario: archived change が即時に一覧から消えない
- **GIVEN** TUI が実行モードである
- **AND** ある change の `queue_status` が `Archived` に更新された
- **WHEN** 画面がレンダリングされる
- **THEN** その change は Changes 一覧に表示されたままである

#### Scenario: 選択モードでも archived change を維持
- **GIVEN** TUI が選択モードに戻った
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change は Changes 一覧に表示されたままである

#### Scenario: TUI 再起動後は archived change が一覧から消える
- **GIVEN** archived change が Changes 一覧に残っている
- **WHEN** TUI を終了して再起動する
- **THEN** archived change は Changes 一覧に表示されない

### Requirement: Git Repository Detection

Executable CLI orchestration SHALL require a usable Git repository and Git command. Validation SHALL happen before orchestration side effects.

#### Scenario: Git repository unavailable

- **WHEN** user starts `cflx run --all` outside a usable Git repository
- **THEN** the command exits non-zero with an actionable Git error
- **AND** no hook, lifecycle adapter, AI subprocess, or workspace mutation starts

### Requirement: TUIのChange一覧にworktree存在を表示する
TUIのChange一覧は、各changeに紐づくworktreeの有無を識別できるインジケータを表示しなければならない（SHALL）。

#### Scenario: worktreeが存在するchangeの表示
- **GIVEN** 対象changeにworktreeが存在する
- **WHEN** TUIのChange一覧を表示する
- **THEN** そのchangeの行にworktreeインジケータが表示される

#### Scenario: worktreeが存在しないchangeの表示
- **GIVEN** 対象changeにworktreeが存在しない
- **WHEN** TUIのChange一覧を表示する
- **THEN** そのchangeの行にworktreeインジケータは表示されない

### Requirement: 実行中changeのworktree削除を禁止する
TUIはProcessing/Running中のchangeに対してworktree削除を許可してはならない（MUST NOT）。
ただし、削除対象worktreeがChanges一覧に存在しない、またはNotQueuedのchangeに紐づく場合は、実行中であっても削除を許可しなければならない（MUST）。

#### Scenario: 実行中の未関連worktreeを削除できる
- **GIVEN** TUIがRunning中である
- **AND** 選択中worktreeがChanges一覧に存在しない、またはNotQueuedのchangeに紐づく
- **WHEN** WorktreesビューでDキーを押して削除を確認する
- **THEN** worktree削除が実行される
- **AND** 削除後にworktree一覧が更新される

#### Scenario: 実行中のqueued/processing系worktreeは削除できない
- **GIVEN** TUIがRunning中である
- **AND** 選択中worktreeがQueued/Processing/Archiving/Resolving/Accepting/MergeWaitのchangeに紐づく
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 削除は行われず、禁止メッセージが表示される

### Requirement: Serial Apply Iteration WIP Commits

Managed-worktree apply execution MUST preserve each iteration's latest work as a new WIP snapshot commit after successful, failed, or no-progress apply results. The message MUST use `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})`; the snapshot MUST be equivalent to `git add -A` followed by `git commit --no-verify --allow-empty`; and it MUST NOT amend an existing WIP commit.

Only when Conflux-owned WIP `git add -A` or commit cannot create an `index.lock` because the current managed worktree Git directory already contains that lock, Conflux MUST retry the complete `create_progress_commit` snapshot sequence up to three total attempts at fixed 200 millisecond intervals without backoff. It MUST NOT delete or bypass the lock and MUST NOT apply this retry policy to general Git commands.

Before each attempt Conflux MUST record `HEAD_before`. A failed attempt counts as committed only when HEAD advanced to a commit whose sole parent is `HEAD_before` and whose subject exactly matches the expected WIP message. A same-subject commit elsewhere in history MUST NOT prove success. Runtime MUST check cancellation after a retryable failure, before waiting, and before each next attempt. Exhausted lock contention and non-lock VCS errors MUST preserve workspace content and return actionable diagnostics.

#### Scenario: Managed-worktree iterations always snapshot current work

- **GIVEN** a managed-worktree apply iteration finishes successfully, fails, or makes no task progress
- **WHEN** iteration finalization runs
- **THEN** current staged and unstaged work is saved in exactly one new WIP snapshot commit
- **AND** no existing WIP commit is amended

#### Scenario: Transient index lock clears within retry budget

- **GIVEN** a Conflux-owned WIP snapshot reports an existing `index.lock` in the current managed worktree Git directory
- **AND** the lock becomes available before the third total attempt
- **WHEN** Conflux retries the complete snapshot sequence after each fixed 200 millisecond delay
- **THEN** the expected WIP commit is created exactly once
- **AND** Conflux does not delete or bypass the lock

#### Scenario: Ambiguous commit completion does not duplicate WIP

- **GIVEN** a WIP attempt captured `HEAD_before` and then reported failure
- **AND** current HEAD advanced to a commit whose sole parent is `HEAD_before` and whose subject exactly matches the expected WIP message
- **WHEN** Conflux evaluates another attempt
- **THEN** it recognizes the prior attempt as committed
- **AND** it does not create a duplicate WIP commit

#### Scenario: Persistent lock and cancellation preserve workspace state

- **GIVEN** lock contention exhausts three attempts or cancellation is observed before another attempt
- **WHEN** progress-commit orchestration stops
- **THEN** no further attempt starts
- **AND** workspace content and the lock file remain untouched
- **AND** diagnostics identify the command, working directory, contention, and attempts

#### Scenario: Non-lock VCS failure is not retried

- **GIVEN** a WIP snapshot fails because of a permission, identity, configuration, hook, conflict, or other non-lock VCS error
- **WHEN** Conflux classifies the failure
- **THEN** it returns the structured VCS failure without transient-lock retry

### Requirement: Archive Context History

オーケストレータは、逐次/並列のどちらの archive でも共通ループで同一の履歴注入ロジックを使用し、各 archive 試行の結果をキャプチャして同じ change に対する後続の archive プロンプトに含めなければならない（MUST）。

#### Scenario: 初回 archive 試行には履歴がない

- **WHEN** オーケストレータが change に対して初めて archive を実行する
- **THEN** プロンプトには設定からの基本 archive_prompt のみが含まれる
- **AND** `<last_archive>` タグは含まれない

#### Scenario: 2回目の archive には前回の試行結果が含まれる

- **GIVEN** change に対する archive の1回目の試行が検証失敗した
- **WHEN** オーケストレータが同じ change に対して2回目の archive を実行する
- **THEN** プロンプトには基本 archive_prompt が含まれる
- **AND** プロンプトには `<last_archive attempt="1">` ブロックが含まれる
- **AND** ブロックには試行回数、成功/失敗ステータス、所要時間、検証結果が含まれる

#### Scenario: 複数の前回試行が含まれる

- **GIVEN** change に対する archive が2回失敗している
- **WHEN** オーケストレータが同じ change に対して3回目の archive を実行する
- **THEN** プロンプトには `<last_archive attempt="1">` と `<last_archive attempt="2">` の両方のブロックが含まれる
- **AND** 各ブロックにはそれぞれの試行の詳細が含まれる

#### Scenario: 履歴は change 完了時にクリアされる

- **GIVEN** change に対する archive 履歴が存在する
- **WHEN** archive が成功し、change が完全に処理される
- **THEN** その change の archive 履歴はクリアされる
- **AND** 次に同じ change ID が処理される場合、履歴は空の状態から始まる

#### Scenario: parallel の2回目 archive に履歴が含まれる
- **GIVEN** parallel mode で change が archive 実行中である
- **AND** 1回目の archive が検証失敗している
- **WHEN** 2回目の archive が実行される
- **THEN** プロンプトは base archive_prompt を含む
- **AND** プロンプトは `<last_archive attempt="1">` ブロックを含む
- **AND** ブロックには 1回目の試行結果が含まれる

### Requirement: Archive History Context Format

archive 履歴コンテキストは、XML 風のタグ形式で構造化されなければならない（SHALL）。

各試行ブロックは以下の情報を含む：
- `attempt`: 試行回数（1-based）
- `status`: success または failed
- `duration`: 所要時間（秒単位）
- `verification_result`: 検証結果（検証失敗時の理由）
- `error`: エラーメッセージ（失敗時）
- `exit_code`: 終了コード

#### Scenario: 検証失敗時の履歴フォーマット

- **GIVEN** archive コマンドは成功したが検証が失敗した
- **WHEN** 履歴コンテキストがフォーマットされる
- **THEN** ブロックには `status: failed` が含まれる
- **AND** `verification_result` には「Change still exists at openspec/changes/{change_id}」などの具体的な理由が含まれる
- **AND** `exit_code: 0` が含まれる（コマンド自体は成功したため）

#### Scenario: コマンド失敗時の履歴フォーマット

- **GIVEN** archive コマンドが失敗した
- **WHEN** 履歴コンテキストがフォーマットされる
- **THEN** ブロックには `status: failed` が含まれる
- **AND** `error` には終了コードに関する情報が含まれる
- **AND** `exit_code` には非ゼロの値が含まれる

### Requirement: Resolve Continuation Context

resolve コマンドの再試行時、システムは前回の試行結果と継続理由をプロンプトに含めなければならない（MUST）。

#### Scenario: 初回 resolve 試行にはコンテキストがない

- **WHEN** システムが conflict resolution のために resolve を初めて実行する
- **THEN** プロンプトには基本的な VCS 状態とコンフリクト情報のみが含まれる
- **AND** `<resolve_context>` ブロックは含まれない

#### Scenario: 2回目の resolve には前回の結果と継続理由が含まれる

- **GIVEN** resolve の1回目の試行後もコンフリクトが残っている
- **WHEN** システムが2回目の resolve を実行する
- **THEN** プロンプトには `<resolve_context>` ブロックが含まれる
- **AND** ブロックには現在の試行番号（"attempt 2 of 3"）が含まれる
- **AND** 前回の試行の結果（コマンド終了ステータス、検証結果）が含まれる
- **AND** 検証失敗の具体的な理由（"Conflicts still present: src/main.rs"）が含まれる
- **AND** 所要時間が含まれる

#### Scenario: マージ未完了による継続理由

- **GIVEN** resolve コマンドが成功終了した
- **AND** しかし `MERGE_HEAD` が存在する（マージ未完了）
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Merge still in progress (MERGE_HEAD exists)」という理由が含まれる

#### Scenario: マージコミット不足による継続理由

- **GIVEN** resolve コマンドが成功終了した
- **AND** しかし必要なマージコミット（"Merge change: {change_id}"）が不足している
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Missing merge commits for change_ids」という理由が含まれる
- **AND** 不足している change_id のリストが含まれる

#### Scenario: Worktree マージ未完了による継続理由

- **GIVEN** 並列実行モードで resolve コマンドが成功終了した
- **AND** しかし worktree でマージが未完了（worktree に `MERGE_HEAD` が存在）
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Worktree merge still in progress for '{revision}'」という理由が含まれる

#### Scenario: Pre-sync コミットサブジェクト不正による継続理由

- **GIVEN** 並列実行モードで resolve コマンドが成功終了した
- **AND** しかし pre-sync マージコミットのサブジェクトが期待と異なる
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Invalid pre-sync merge commit subject」という理由が含まれる
- **AND** 期待されるサブジェクトと実際のサブジェクトが含まれる

### Requirement: Resolve Context Format

When resolve verification continues because merge completion is still incomplete, the continuation reason SHALL distinguish between true missing merge evidence and successful fast-forward integration.

#### Scenario: Fast-forward merge does not emit missing-merge-commits context

- **GIVEN** the resolve command exits successfully
- **AND** the change has been integrated into the base branch via fast-forward
- **WHEN** the system evaluates whether another resolve attempt is needed
- **THEN** `<resolve_context>` does not include `Missing merge commits for change_ids`
- **AND** the change is not scheduled for another resolve attempt based on merge-commit absence alone

### Requirement: Enhanced Help Output

CLI help SHALL document all current subcommands and supported options. It SHALL include `--max-concurrent`, `--dry-run`, `--vcs`, web controls, and `--push [remote]` where accepted, and SHALL NOT advertise `--parallel`.

#### Scenario: Run help describes standard worktree execution

- **WHEN** user runs `cflx run --help`
- **THEN** help describes explicit targets, concurrency, dry-run, VCS, web monitoring, and post-archive options
- **AND** it contains no `--parallel` option or mode-selection example

### Requirement: Deprecated Flags Removed

The CLI SHALL NOT accept --opencode-path or --openspec-cmd flags.

#### Scenario: --opencode-path flag rejected

- **WHEN** user runs `cflx --opencode-path /path/to/opencode`
- **THEN** CLI exits with error
- **AND** error message indicates flag is not recognized

#### Scenario: --openspec-cmd flag rejected

- **WHEN** user runs `cflx --openspec-cmd custom-cmd`
- **THEN** CLI exits with error
- **AND** error message indicates flag is not recognized

#### Scenario: OPENSPEC_CMD environment variable not used

- **WHEN** OPENSPEC_CMD environment variable is set
- **THEN** CLI does not read or use this environment variable
- **AND** configuration file settings are used instead

### Requirement: changes間のspec delta衝突検出コマンド
CLI SHALL provide a subcommand to detect conflicts between spec delta files across changes without using an LLM.

#### Scenario: 衝突なしの場合の成功
- **WHEN** user runs the new conflict detection command
- **AND** no conflicting spec deltas are found
- **THEN** the command exits with status code 0

#### Scenario: 衝突が検出された場合
- **WHEN** user runs the new conflict detection command
- **AND** conflicting spec deltas are found
- **THEN** the command outputs conflict details
- **AND** the command exits with a non-zero status code

#### Scenario: JSON出力の指定
- **WHEN** user runs the new conflict detection command with a JSON output flag
- **THEN** the command outputs a machine-readable JSON payload

### Requirement: Web Execution Control Availability
Web UIからの実行制御は、`--web` でHTTPサーバーが起動している場合にのみ有効でなければならない（SHALL）。TUIとRunモードのいずれでも同じ制御経路を提供しなければならない（MUST）。RunモードではTUIと同等のリトライ/停止挙動を提供しなければならない（SHALL）。

#### Scenario: TUIモードでのWeb制御
- **GIVEN** `cflx tui --web` で起動している
- **WHEN** Web UI が制御APIへ開始/停止要求を送る
- **THEN** TUIの実行状態が同等に変化する

#### Scenario: RunモードでのWeb制御
- **GIVEN** `cflx run --web` で起動している
- **WHEN** Web UI が制御APIへ開始/停止要求を送る
- **THEN** オーケストレーターの実行状態が同等に変化する

#### Scenario: Runモードでのリトライ制御
- **GIVEN** `cflx run --web` で実行中にエラーが発生している
- **WHEN** Web UI が制御APIへ retry 要求を送る
- **THEN** オーケストレーターは同一のエラー change を再実行する

### Requirement: 選択中worktreeの削除操作を提供する

TUIは通常削除確認と既知dirty内容の破棄確認を区別しなければならない（MUST）。通常確認の`Y`または`S`はdirty-discard permissionを付与してはならない（MUST NOT）。fresh service observationが既知dirtyを返した場合のみ第二確認を表示し、大文字`X`だけを破棄入力として受理しなければならない（MUST）。`S`はskip-teardown選択だけを表し、第二確認まで保持されるが、それ自体はdirty削除を許可してはならない（MUST NOT）。

#### Scenario: Dキーで削除確認を出す
- **GIVEN** TUIがWorktreesビューである
- **AND** 選択中worktreeが削除可能である
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 通常削除確認が表示される
- **AND** `Y`はteardownあり、`S`はskip-teardownであることが表示される

#### Scenario: Yでclean worktreeを削除する
- **GIVEN** 通常削除確認の対象がcleanで削除適格である
- **WHEN** ユーザーが`Y`を押す
- **THEN** teardown後に対象worktreeが削除される

#### Scenario: Yからdirty破棄確認へ進む
- **GIVEN** fresh service observationが対象を既知dirtyと判定する
- **WHEN** 通常確認で`Y`を押す
- **THEN** worktreeはまだ削除されない
- **AND** skip-teardown=falseを保持したdirty破棄確認が表示される
- **WHEN** ユーザーが大文字`X`を押す
- **THEN** teardownと最終再検証後にworktreeが削除される

#### Scenario: Sからdirty破棄確認へ進む
- **GIVEN** fresh service observationが対象を既知dirtyと判定する
- **WHEN** 通常確認で`S`を押す
- **THEN** worktreeはまだ削除されない
- **AND** skip-teardown=trueを保持し、teardownも省略されることを示すdirty破棄確認が表示される
- **WHEN** ユーザーが大文字`X`を押す
- **THEN** teardownを実行せず、最終再検証後にworktreeが削除される

#### Scenario: dirty破棄確認はX以外で削除しない
- **GIVEN** dirty破棄確認が表示されている
- **WHEN** ユーザーが`Y`、`S`、小文字`x`、または無関係なキーを押す
- **THEN** 削除は実行されない
- **WHEN** ユーザーが`N`またはEscを押す
- **THEN** 確認は閉じ、worktreeは保持される

#### Scenario: unknown observationは破棄確認へ進まない
- **GIVEN** dirty、commits-ahead、base merge、Git identity、またはbranch refの安全観測を確定できない
- **WHEN** 通常削除またはdirty破棄が要求される
- **THEN** dirty破棄確認へ進まず削除を拒否する
- **AND** 理由を表示する

#### Scenario: dispatch前のactive遷移を拒否する
- **GIVEN** dirty破棄確認が表示されている
- **WHEN** 対象changeがdispatch前にactiveまたはdeletingへ遷移する
- **THEN** 削除を拒否しworktreeを保持する

#### Scenario: worktree一覧が空の場合の削除操作
- **GIVEN** TUIがWorktreesビューである
- **AND** worktree一覧が空である
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 何も起こらない

### Requirement: Changes一覧ログプレビューの相対時間表記
TUIのChanges一覧に表示されるログプレビューは、相対時間を括弧で囲んだ形式で表示しなければならない（SHALL）。

#### Scenario: 相対時間を括弧で囲む
- **GIVEN** Changes一覧にログプレビューが表示される
- **WHEN** TUIがChanges一覧を描画する
- **THEN** ログプレビューの相対時間は括弧付き形式（例: `(2m ago)`）で表示される

### Requirement: カーソル行のログプレビュー視認性
TUIのChanges一覧でカーソル行が選択されている場合、ログプレビューの文字色は非選択行より明るく表示しなければならない（SHALL）。

#### Scenario: カーソル行でログプレビューが判読できる
- **GIVEN** Changes一覧のカーソル行が選択されている
- **AND** 該当行にログプレビューが表示されている
- **WHEN** TUIがChanges一覧を描画する
- **THEN** ログプレビューは選択背景上でも判読できる明るい文字色で表示される

### Requirement: Running Changes一覧の経過時間配置
TUIのRunningモードにおけるChanges一覧は、in-flight状態（Applying/Accepting/Archiving/Resolving）の行で、動作中スピナーの直後に経過時間を表示しなければならない（SHALL）。経過時間はステータス表示の前に配置しなければならない（SHALL）。

#### Scenario: in-flight行でスピナー直後に経過時間を表示する
- **GIVEN** TUIがRunningモードである
- **AND** changeのqueue_statusがApplyingである
- **AND** changeの開始時刻が取得できる
- **WHEN** TUIがChanges一覧を描画する
- **THEN** change行の表示はスピナーの直後に経過時間を含む
- **AND** 経過時間はステータス表示の前に配置される

### Requirement: Log Panel Visibility Toggle
TUI は Changes ビューで `l` キーによりログパネルの表示/非表示を切り替えられるようにしなければならない（SHALL）。
ログパネルの既定状態は表示（有効）でなければならない（SHALL）。

#### Scenario: Toggle off hides log panel while keeping logs
- **GIVEN** ログパネルが有効である
- **AND** ログが存在する
- **WHEN** ユーザーが `l` キーを押す
- **THEN** ログパネルは非表示になる
- **AND** 新しいログは引き続きログバッファに追加される

#### Scenario: Toggle on restores log panel
- **GIVEN** ログパネルが無効である
- **WHEN** ユーザーが `l` キーを押す
- **THEN** ログが存在する場合、ログパネルが表示される

### Requirement: TUIログファイルの常時出力

TUI のログファイル出力は常時有効でなければならず（MUST）、`tui --logs` オプションは提供してはならない（MUST NOT）。

#### Scenario: `tui --logs` は無効
- **WHEN** ユーザーが `cflx tui --logs /tmp/debug.log` を実行する
- **THEN** CLI は不明なオプションとしてエラーを表示する
- **AND** 終了コードは非0である

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled Conflux agent skills into standard `.agents/skills` or `.claude/skills` locations without requiring a source argument.

#### Scenario: Install bundled skills in project scope by default

- **WHEN** the user runs `cflx install-skills`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `./.agents/skills`
- **AND** the lock file is written to `./.agents/.skill-lock.json`

#### Scenario: Install bundled skills in global scope

- **WHEN** the user runs `cflx install-skills --global`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `~/.agents/skills`
- **AND** the lock file is written to `~/.agents/.skill-lock.json`

#### Scenario: Install bundled skills in Claude project scope

- **WHEN** the user runs `cflx install-skills --claude`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `./.claude/skills`
- **AND** the lock file is written to `./.claude/.skill-lock.json`

#### Scenario: Install bundled skills in Claude global scope

- **WHEN** the user runs `cflx install-skills --claude --global`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `~/.claude/skills`
- **AND** the lock file is written to `~/.claude/.skill-lock.json`

#### Scenario: Reject legacy explicit self source syntax

- **WHEN** the user runs `cflx install-skills self`
- **THEN** the command exits with an error
- **AND** the error message instructs the user to run `cflx install-skills` or `cflx install-skills --global`

#### Scenario: Reject unsupported explicit local source syntax

- **WHEN** the user runs `cflx install-skills local:../my-skills`
- **THEN** the command exits with an error
- **AND** the error message instructs the user to run `cflx install-skills` or `cflx install-skills --global`

### Requirement: run Surfaces Hook Output

The `run` subcommand SHALL preserve hook command, output, and failure ordering on the sole worktree execution path.

#### Scenario: CLI run preserves hook visibility

- **GIVEN** a hook is configured for a lifecycle stage reached during `cflx run`
- **WHEN** the hook executes
- **THEN** users see the hook command followed by captured output and any failure result
- **AND** debug-only tracing is not required

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled Conflux agent skills into standard skill locations without requiring a source argument.

#### Scenario: Install bundled skills in project scope by default

- **WHEN** the user runs `cflx install-skills`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `./.agents/skills`
- **AND** the lock file is written to `./.agents/.skill-lock.json`

#### Scenario: Install bundled skills in global scope

- **WHEN** the user runs `cflx install-skills --global`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `~/.agents/skills`
- **AND** the lock file is written to `~/.agents/.skill-lock.json`

#### Scenario: Install bundled skills into Claude project scope

- **WHEN** the user runs `cflx install-skills --claude`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `./.claude/skills`
- **AND** the lock file is written to `./.claude/.skill-lock.json`

#### Scenario: Install bundled skills into Claude global scope

- **WHEN** the user runs `cflx install-skills --claude --global`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `~/.claude/skills`
- **AND** the lock file is written to `~/.claude/.skill-lock.json`

#### Scenario: Reject legacy explicit self source syntax

- **WHEN** the user runs `cflx install-skills self`
- **THEN** the command exits with an error
- **AND** the error message instructs the user to run `cflx install-skills` or `cflx install-skills --global`

#### Scenario: Reject unsupported explicit local source syntax

- **WHEN** the user runs `cflx install-skills local:../my-skills`
- **THEN** the command exits with an error
- **AND** the error message instructs the user to run `cflx install-skills` or `cflx install-skills --global`

## Requirements

### Requirement: TUI change editor launch resolves archived changes

TUI の Changes view で `e` キーによる change editor launch を実行する際、システムは active change に加えて `openspec/changes/archive/` 配下の archived change entry も解決しなければならない（MUST）。

archive entry の解決では direct match (`openspec/changes/archive/<change_id>`) と date-prefixed match (`openspec/changes/archive/<date>-<change_id>`) の両方を同一 change として扱わなければならない（MUST）。

解決済み entry に `proposal.md` が存在する場合はそのファイルを editor で開き、存在しない場合は change directory 自体を editor のカレントディレクトリとして開かなければならない（MUST）。

#### Scenario: e key opens archived change proposal from dated archive entry

- **GIVEN** TUI の Changes view で selected change id が `fix-archived-editor` である
- **AND** active path `openspec/changes/fix-archived-editor` は存在しない
- **AND** archive path `openspec/changes/archive/2026-04-11-fix-archived-editor/proposal.md` が存在する
- **WHEN** ユーザーが `e` キーを押す
- **THEN** system は archive path を selected change の実体として解決する
- **AND** `proposal.md` を editor launch 対象として使用する
- **AND** `ChangeNotFound` を返さない

#### Scenario: e key still prefers active change path

- **GIVEN** TUI の Changes view で selected change id が `active-change` である
- **AND** `openspec/changes/active-change/proposal.md` が存在する
- **AND** `openspec/changes/archive/2026-04-11-active-change/` も存在する
- **WHEN** ユーザーが `e` キーを押す
- **THEN** system は active path を優先して解決する
- **AND** active change の `proposal.md` を editor launch 対象として使用する

## Requirements

### Requirement: Native OpenSpec utility subcommands

The CLI SHALL provide a native `cflx openspec` command namespace for repository-scoped OpenSpec utility operations that were previously exposed through skill-local helper scripts. These subcommands MUST support `list`, `show`, `validate`, and `archive` without requiring a bundled Python helper.

When the user runs `cflx openspec list` without `--specs`, the command MUST enumerate only non-archived change entries from `openspec/changes/` and MUST NOT include archived change entries from `openspec/changes/archive/` in the human-readable change list.

For each listed active change that declares proposal dependencies, the human-readable change list MUST render a `Dependencies:` line. Each dependency entry MUST include the dependency id and a status label in the form `<dependency-id> [<status>]`.

For active changes that declare proposal dependencies, `cflx openspec show <change-id>` MUST render dependency status details in human-readable output using the same `<dependency-id> [<status>]` format as list output.

For active changes that declare proposal dependencies, `cflx openspec show --json <change-id>` MUST expose dependency status details as structured JSON data containing each dependency id and status label.

The status label MUST be derived from workspace-local repository evidence as follows:

- `done` when the dependency target is archived under `openspec/changes/archive/`, including dated archive directory names whose date prefix maps to the dependency id
- `running` when the dependency target is listed in `.conflux-inflight`
- `rejected` when the dependency target has `openspec/changes/<id>/proposal.md` and `openspec/changes/<id>/REJECTED.md`
- `pending` when the dependency target exists as an active change under `openspec/changes/` and is not classified as `running`, `done`, or `rejected`
- `missing` when the dependency target is not found as active, in-flight, rejected, or archived

The list command and human-readable show command MUST omit the `Dependencies:` line for active changes that declare no dependencies.

The `cflx openspec show --deltas-only <change-id>` output MUST remain focused on spec deltas and MUST NOT add dependency status details.

Archived changes MAY still be resolved by detail-oriented subcommands such as `cflx openspec show <change-id>`, but nested archive paths such as `openspec/changes/archive/YYYY-MM-DD/<change-id>` MUST NOT be resolved as archived changes.

The native `cflx openspec archive <change-id>` subcommand MUST archive successful changes into a date-prefixed destination under `openspec/changes/archive/` using the format `YYYY-MM-DD-<change-id>`. Pre-archive validation errors or an invalid result MUST prevent archive mutation, while advisory validation warnings alone MUST NOT prevent archive.

#### Scenario: show displays pending active dependency

- **GIVEN** active change `feature-b` declares dependency `feature-a`
- **AND** `openspec/changes/feature-a/proposal.md` exists
- **AND** `feature-a` is not listed in `.conflux-inflight`
- **AND** no archive entry or `REJECTED.md` marker for `feature-a` exists
- **WHEN** the user runs `cflx openspec show feature-b`
- **THEN** the output includes `Dependencies: feature-a [pending]`

#### Scenario: show displays rejected dependency

- **GIVEN** active change `feature-b` declares dependency `feature-a`
- **AND** `openspec/changes/feature-a/proposal.md` exists
- **AND** `openspec/changes/feature-a/REJECTED.md` exists
- **WHEN** the user runs `cflx openspec show feature-b`
- **THEN** the output includes `feature-a [rejected]` in its `Dependencies:` line

#### Scenario: archive subcommand stores change in dated archive directory

- **GIVEN** active change `add-env-openspec-cmd` exists under `openspec/changes/add-env-openspec-cmd`
- **WHEN** the user runs `cflx openspec archive add-env-openspec-cmd`
- **THEN** the active change directory is removed from `openspec/changes/add-env-openspec-cmd`
- **AND** the archived change exists at `openspec/changes/archive/2026-01-08-add-env-openspec-cmd`
- **AND** the success output reports `openspec/changes/archive/2026-01-08-add-env-openspec-cmd`

#### Scenario: archive subcommand fails when dated destination already exists

- **GIVEN** active change `add-env-openspec-cmd` exists under `openspec/changes/add-env-openspec-cmd`
- **AND** `openspec/changes/archive/2026-01-08-add-env-openspec-cmd` already exists
- **WHEN** the user runs `cflx openspec archive add-env-openspec-cmd`
- **THEN** the command fails with an archive destination already exists error
- **AND** the active change directory is not silently moved to another generated name

#### Scenario: show rejects nested archive layout

- **GIVEN** `openspec/changes/archive/2026-07-09/feature-a/proposal.md` exists
- **AND** no valid archive entry exists at `openspec/changes/archive/feature-a` or `openspec/changes/archive/2026-07-09-feature-a`
- **WHEN** the user runs `cflx openspec show feature-a`
- **THEN** the command does not resolve `feature-a` as an archived change
- **AND** the command reports invalid archive layout instead of treating the nested directory as valid

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled Conflux agent skills into standard skill locations without requiring a source argument.

#### Scenario: Install bundled skills includes router and operation-specific skills

- **WHEN** the user runs `cflx install-skills`
- **THEN** the installed bundled skill set includes `cflx-proposal`, `cflx-run`, `cflx-workflow`, `cflx-analyze`, `cflx-apply`, `cflx-rejecting`, `cflx-cleanup-review`, `cflx-accept`, `cflx-archive`, and `cflx-resolve`
- **AND** `cflx-workflow` is installed as a backward-compatible router alongside the new operation-specific skills

#### Scenario: Install bundled skills preserves per-skill auxiliary files and self-contained router compatibility

- **WHEN** the user runs `cflx install-skills`
- **THEN** each operation-specific skill installs the auxiliary files required for that operation within its own skill directory
- **AND** bundled skill installation does not reintroduce `scripts/cflx.py`
- **AND** `cflx-workflow` remains installable as a self-contained compatibility router
- **AND** legacy prompts that load only `cflx-workflow` do not require cross-skill auxiliary file access after installation

### Requirement: CLI startup behavior emits versioned startup logs

The CLI SHALL emit a versioned startup log for each primary runtime entry path covered by this change: `run` and `server`.

Versioned startup logs MUST use a consistent version/build representation so operators can compare startup logs across modes without interpreting different formats.

#### Scenario: Run subcommand emits versioned startup log
- **GIVEN** a user runs `cflx run`
- **WHEN** the CLI starts the orchestration runtime
- **THEN** the startup log includes the cflx version and build number
- **AND** the startup log identifies the mode as run

#### Scenario: Server subcommand emits versioned startup log
- **GIVEN** a user runs `cflx server`
- **WHEN** the CLI starts the server runtime
- **THEN** the startup log includes the cflx version and build number
- **AND** the startup log identifies the mode as server

### Requirement: CLI acceptance failure reporting distinguishes verdict failure from follow-up persistence degradation

When the CLI acceptance loop receives a non-pass acceptance verdict, it SHALL distinguish the acceptance diagnosis from any later persistence problem while recording follow-up tasks.

A failure to append findings into `tasks.md` MAY be reported as warning or supplemental execution context, but it MUST NOT replace the acceptance verdict as the primary reported reason unless the verdict itself could not be determined.

#### Scenario: CLI keeps acceptance fail as primary reason when persistence degrades
- **GIVEN** the acceptance command returns `FAIL` with concrete findings
- **AND** follow-up persistence into `tasks.md` later fails
- **WHEN** the CLI reports the acceptance result
- **THEN** the primary reported outcome is still acceptance `FAIL`
- **AND** the `tasks.md` persistence problem is reported separately as supplemental context

### Requirement: Native OpenSpec validator must not infer proposal quality from free-text wording

The native OpenSpec validator SHALL validate proposal/task quality only from explicit, parseable structure such as declared metadata fields, verification ownership markers, or other machine-readable syntax defined by the canonical specs.

The validator MUST NOT classify proposal intent or implementation adequacy solely from keyword matches in free-text task or proposal prose.

#### Scenario: wording variation does not change validator outcome without structural change
- **GIVEN** two proposals have the same explicit metadata and verification markers
- **AND** they differ only in free-text phrasing or synonymous wording
- **WHEN** the native validator evaluates them
- **THEN** the validator returns the same structural validation outcome for both
- **AND** it does not emit different quality warnings based only on keyword wording

### Requirement: Canonical spec promotion must fail closed on malformed delta structure

When a spec delta cannot be parsed into the canonical requirement block structure required for promotion, canonicalization SHALL fail with a deterministic parse/promotion error.

The promotion engine MUST NOT rewrite malformed delta text into a best-effort canonical spec as a fallback.

#### Scenario: malformed delta does not fallback-rewrite into canonical text
- **GIVEN** a change spec delta lacks parseable requirement blocks for canonical promotion
- **WHEN** the promotion engine attempts canonicalization
- **THEN** the engine returns a deterministic parse or promotion error
- **AND** it does not rewrite section markers to synthesize a best-effort canonical spec

### Requirement: Logs Command

The Conflux CLI SHALL expose a `logs` subcommand for read-only access to persistent Conflux log files.

The command SHALL support printing the selected path, printing a bounded recent tail, following appended lines, preferring today's log file, and selecting a log project by explicit project slug.

#### Scenario: Logs command help documents modes

- **GIVEN** the Conflux binary is available
- **WHEN** the user runs `cflx logs --help`
- **THEN** the help output documents path printing
- **AND** the help output documents bounded tail output
- **AND** the help output documents follow mode
- **AND** the help output documents today's log preference
- **AND** the help output documents explicit project slug selection

#### Scenario: Logs command is read-only

- **GIVEN** persistent Conflux logs already exist
- **WHEN** the user runs `cflx logs --path` or `cflx logs --last 1`
- **THEN** the command reads or reports log locations only
- **AND** the command does not initialize the normal runtime file log sink for the purpose of viewing logs

### Requirement: Shell Completion Generation

The CLI SHALL provide a `completion` subcommand that generates shell completion scripts for supported shells without starting orchestration, TUI, server, or workspace-dependent runtime behavior during script generation.

<!-- Expected canonical result after archive: cli spec documents `cflx completion <shell>` as a side-effect-free script generation surface for zsh, bash, fish, and powershell. -->

#### Scenario: Generate completion script for supported shell

- **WHEN** user runs `cflx completion zsh`
- **OR** user runs `cflx completion bash`
- **OR** user runs `cflx completion fish`
- **OR** user runs `cflx completion powershell`
- **THEN** `cflx` prints a non-empty completion script to stdout
- **AND** exits with status code 0
- **AND** does not create or append Conflux log files
- **AND** does not require an OpenSpec workspace

#### Scenario: Completion output is script-only

- **WHEN** user runs `cflx completion <supported-shell>`
- **THEN** stdout contains the generated completion script
- **AND** stdout does not contain startup logs, status banners, or human-readable explanatory text

#### Scenario: Unsupported completion shell is rejected

- **WHEN** user runs `cflx completion unsupported-shell`
- **THEN** the command exits with a non-zero status code
- **AND** the error output lists the supported shell values

### Requirement: Dynamic Change ID Completion

Generated shell completion scripts SHALL provide workspace-local OpenSpec change ID candidates for commands and options that accept change IDs. Candidate lookup SHALL be side-effect free and SHALL read only workspace-local `openspec/changes/` state.

<!-- Expected canonical result after archive: cli spec documents dynamic OpenSpec change-id completion for `run --change`, `openspec show`, `openspec validate`, and `openspec archive`, including active/archived scoping and dated archive normalization. -->

#### Scenario: Run change option completes active changes

- **GIVEN** active changes exist under `openspec/changes/`
- **WHEN** the user requests shell completion for `cflx run --change <prefix>`
- **THEN** completion candidates include matching active change IDs
- **AND** archived changes are not included

#### Scenario: Run change option completes comma-separated values

- **GIVEN** active changes `alpha`, `beta`, and `gamma` exist
- **WHEN** the user requests shell completion for `cflx run --change alpha,b`
- **THEN** completion candidates are evaluated against the current comma-separated token `b`
- **AND** `beta` is offered as a candidate
- **AND** already-entered values such as `alpha` are not duplicated when the shell integration can suppress duplicates

#### Scenario: Openspec show completes active and archived changes

- **GIVEN** active change `active-change` exists
- **AND** archived change `archived-change` exists under `openspec/changes/archive/`
- **WHEN** the user requests shell completion for `cflx openspec show <prefix>`
- **THEN** completion candidates include matching active changes
- **AND** completion candidates include matching archived changes

#### Scenario: Openspec show normalizes dated archived change IDs

- **GIVEN** archived change directory `openspec/changes/archive/2026-04-28-archived-change` exists
- **WHEN** the user requests shell completion for `cflx openspec show archived`
- **THEN** the candidate is `archived-change`
- **AND** the date prefix is not included in the displayed logical change ID

#### Scenario: Openspec validate completes active changes only

- **GIVEN** active and archived changes exist
- **WHEN** the user requests shell completion for `cflx openspec validate <prefix>`
- **THEN** completion candidates include matching active changes
- **AND** archived changes are not included
- **AND** invoking `cflx openspec validate` with no change ID remains valid

#### Scenario: Openspec archive completes active changes only

- **GIVEN** active and archived changes exist
- **WHEN** the user requests shell completion for `cflx openspec archive <prefix>`
- **THEN** completion candidates include matching active changes
- **AND** archived changes are not included

#### Scenario: Change ID candidate lookup is side-effect free

- **WHEN** a generated completion script asks `cflx` for change ID candidates
- **THEN** the candidate lookup reads only workspace-local `openspec/changes/` state
- **AND** does not initialize runtime logging
- **AND** does not create, update, or delete workflow state
- **AND** exits with status code 0 and empty stdout when no workspace or no candidates exist

### Requirement: Repository-Scoped Orchestration Lock

Conflux MUST allow at most one local orchestration-owning process for a Git repository at a time. Repository identity MUST be based on the canonical Git common directory so linked worktrees share the same exclusion scope. Ownership MUST use an OS-managed, non-blocking process lock retained for the process lifetime; diagnostic file contents MUST NOT determine lock ownership or workflow state.

#### Scenario: Competing process in the same repository is rejected

- **GIVEN** a local `cflx run`, local TUI, or `cflx server` process owns the repository lock
- **WHEN** another local orchestration-owning invocation targets the same Git common directory
- **THEN** the second invocation exits non-zero before starting orchestration, API listeners, lifecycle adapters, or AI subprocesses
- **AND** the owning process continues unaffected

#### Scenario: Linked worktrees share one lock

- **GIVEN** two worktrees resolve to the same canonical Git common directory
- **AND** one worktree has a local orchestration-owning Conflux process
- **WHEN** local orchestration is started from the other worktree
- **THEN** the second invocation is rejected as a repository lock conflict

#### Scenario: Different repositories run concurrently

- **GIVEN** two working directories resolve to different canonical Git common directories
- **WHEN** local orchestration is started in both directories
- **THEN** each process may acquire its own repository lock

#### Scenario: Process termination releases ownership

- **GIVEN** a process owns a repository lock
- **WHEN** that process exits normally or is terminated abnormally
- **THEN** the OS releases the lock with the owning file descriptor
- **AND** a later local orchestration invocation can acquire the lock even if diagnostic metadata remains

#### Scenario: Non-owning commands remain available

- **GIVEN** a process owns a repository lock
- **WHEN** another invocation runs a non-orchestration command or uses TUI remote-client mode
- **THEN** that invocation does not attempt to acquire the local orchestration lock

### Requirement: Repository Lock Conflict Diagnostics

A lock owner MUST publish best-effort diagnostic metadata containing its PID, start time, canonical workspace, invocation mode, and every API endpoint whose listener completed startup. Endpoint metadata MUST distinguish Unix and TCP addresses. Readers MUST accept legacy metadata containing only `api_url`. A conflicting invocation MUST display all valid available owner metadata, omit unavailable endpoints, and remain safe when metadata is missing or malformed. Endpoint metadata MUST NOT control lock ownership or workflow routing.

#### Scenario: Conflict reports default Unix endpoint

- **GIVEN** a process owns the repository lock
- **AND** its default Unix listener successfully bound
- **WHEN** another local orchestration-owning invocation targets the repository
- **THEN** the conflict diagnostic includes `unix://${GIT_COMMON_DIR}/cflx-api.sock`

#### Scenario: Conflict reports dual endpoints

- **GIVEN** a process successfully bound its UDS and `--web` TCP listeners
- **WHEN** another local orchestration-owning invocation targets the repository
- **THEN** the conflict diagnostic includes both actual endpoints
- **AND** an OS-assigned TCP port is reported when the owner requested port `0`

#### Scenario: Legacy single URL metadata remains readable

- **GIVEN** owner metadata was written by a prior version with only `api_url`
- **WHEN** a conflict diagnostic reads it
- **THEN** the legacy URL is reported as an available endpoint
- **AND** it does not affect OS lock ownership

#### Scenario: Partial startup publishes no endpoints

- **GIVEN** one requested listener binds but another requested listener fails during startup
- **WHEN** the process exits the failed startup transaction
- **THEN** owner metadata does not claim either endpoint as active
- **AND** any created listener and owned socket are cleaned up

#### Scenario: Malformed endpoint metadata does not control ownership

- **GIVEN** the repository lock is held but endpoint metadata is absent, incomplete, or malformed
- **WHEN** another invocation attempts startup
- **THEN** the second invocation is rejected because the OS lock is held
- **AND** the diagnostic reports only fields and endpoints that can be read safely

### Requirement: CLI OpenAPI schema export

The CLI MUST provide `cflx openapi` as a read-only command that emits the build's complete OpenAPI 3.1 YAML document to standard output. It MUST use the same generated contract source as the live `/api/v2/openapi.yaml` endpoint, MUST NOT require a Git repository, and MUST NOT start logging, listeners, lifecycle adapters, AI subprocesses, or orchestration. Standard output MUST contain only the schema so shell redirection produces a valid standalone document. Diagnostics MUST use standard error and failures MUST exit non-zero.

#### Scenario: Export schema without a repository

**Given**: `cflx` is built with OpenAPI support and the current directory is not a Git repository
**When**: the operator runs `cflx openapi`
**Then**: the command exits successfully
**And**: stdout parses as a complete OpenAPI 3.1 YAML document
**And**: no repository lock or orchestration service is started

#### Scenario: Redirect schema to a file

**Given**: `cflx` is built with OpenAPI support
**When**: the operator runs `cflx openapi > openapi.yaml`
**Then**: `openapi.yaml` contains only the generated schema
**And**: the document matches the contract served by `/api/v2/openapi.yaml` from the same build

#### Scenario: OpenAPI support is unavailable

**Given**: `cflx` is built without the feature that provides the OpenAPI document
**When**: the operator invokes `cflx openapi`
**Then**: the command exits non-zero
**And**: stderr explains that OpenAPI support is unavailable
**And**: stdout contains no partial schema
