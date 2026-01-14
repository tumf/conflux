# cli Specification

## Purpose
Defines CLI commands, subcommands, flags, and their behaviors for the openspec-orchestrator binary.
## Requirements
### Requirement: Subcommand Structure

CLI SHALL have a subcommand structure that supports future command extensions.

#### Scenario: Run without subcommand

- **WHEN** user runs `openspec-orchestrator` without arguments
- **THEN** the interactive TUI is launched
- **AND** the change list is displayed in selection mode

#### Scenario: Run with unknown subcommand

- **WHEN** user runs with a non-existent subcommand
- **THEN** an error message with available subcommands is displayed

### Requirement: run Subcommand

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.

#### Scenario: Run with specific change

- **WHEN** user runs `openspec-orchestrator run --change <id>`
- **THEN** only the specified change is processed
- **AND** the snapshot log shows only the specified change

#### Scenario: Run with comma-separated changes

- **WHEN** user runs `openspec-orchestrator run --change a,b,c`
- **THEN** only changes `a`, `b`, `c` are processed
- **AND** the snapshot log shows only `a`, `b`, `c`

#### Scenario: Run with non-existent change

- **WHEN** user runs `openspec-orchestrator run --change nonexistent`
- **AND** no change named `nonexistent` exists
- **THEN** a warning message "Specified change 'nonexistent' not found, skipping" is displayed
- **AND** exits with "No changes found"

#### Scenario: Run with mixed valid and invalid changes

- **WHEN** user runs `openspec-orchestrator run --change a,nonexistent,c`
- **AND** `a` and `c` exist but `nonexistent` does not
- **THEN** a warning message "Specified change 'nonexistent' not found, skipping" is displayed
- **AND** only `a` and `c` are processed
- **AND** the snapshot log shows only `a` and `c`

### Requirement: Default TUI Launch

When launched without a subcommand, the interactive TUI SHALL be displayed.

#### Scenario: Launch without subcommand

- **WHEN** user runs `openspec-orchestrator` without arguments
- **THEN** the interactive TUI is launched
- **AND** the change list is displayed in selection mode

#### Scenario: Launch with run subcommand (backward compatibility)

- **WHEN** user runs `openspec-orchestrator run`
- **THEN** the orchestration loop is executed directly as before

### Requirement: Change Selection Mode

At TUI launch, the change selection mode SHALL be displayed, allowing users to select changes for processing.

#### Scenario: Exit

- **WHEN** user presses `q` key or `Ctrl+C`
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

TUI SHALL display a dashboard-style UI in running mode.

#### Scenario: Display on processing completion

- **WHEN** all queued changes have been processed
- **THEN** the header status changes to "Completed"
- **AND** "Done" is displayed in green on the left side of the status panel
- **AND** TUI maintains display, allowing user to exit with `q` key

#### Scenario: Queue modification after completion

- **WHEN** AppMode is Completed
- **AND** user presses Space key
- **THEN** NotQueued changes can be changed to Queued
- **AND** Queued changes can be changed to NotQueued
- **AND** Completed/Archived/Error changes cannot be modified

#### Scenario: Re-run after completion

- **WHEN** AppMode is Completed
- **AND** changes are added to the queue
- **AND** user presses F5 key
- **THEN** AppMode changes to Running
- **AND** processing of queued changes begins

### Requirement: TUI Layout Structure

The TUI SHALL display appropriate layout for Stopping and Stopped modes in addition to existing modes.

#### Scenario: Stopping mode layout

- **WHEN** TUI is in Stopping mode
- **THEN** header displays "Stopping..." status in yellow
- **AND** current processing panel shows "Completing..."
- **AND** log panel is visible with stop messages

#### Scenario: Stopped mode layout

- **WHEN** TUI is in Stopped mode
- **THEN** header displays "Stopped" status in gray
- **AND** status panel shows summary of completed/queued changes
- **AND** footer shows available actions (F5: resume, q: quit)

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

#### Scenario: NEW badge cleared on approval

- **WHEN** user approves a change with NEW badge (via @ key)
- **THEN** the NEW badge is removed
- **AND** the new count in the footer is decremented

#### Scenario: NEW badge cleared on queue addition

- **WHEN** user adds a change with NEW badge to the queue (Running/Stopped mode)
- **THEN** the NEW badge is removed
- **AND** the new count in the footer is decremented

### Requirement: Dynamic Execution Queue

Running 中に queued change を外した場合、当該 change がまだ Processing を開始していないなら、オーケストレータはその change を実行対象から除外しなければならない（MUST）。Processing/Archiving の change は引き続き操作できない。

#### Scenario: Running 中に queued change を外す
- **WHEN** TUI が Running モードである
- **AND** ユーザーが queued change を Space キーで NotQueued に切り替える
- **AND** その change が Processing を開始していない
- **THEN** その change は実行対象から除外される
- **AND** 以降の実行でその change は処理されない

#### Scenario: Processing 中の change は操作できない
- **WHEN** change が Processing または Archiving である
- **THEN** Space キーを押しても selected/queue 状態は変更されない

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

`init` subcommand SHALL generate a `.openspec-orchestrator.jsonc` configuration template file in the current directory.

#### Scenario: Generate default template (claude)

- **WHEN** user runs `openspec-orchestrator init`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Claude Code template
- **AND** the template includes apply_command, archive_command, analyze_command, and hooks

#### Scenario: Generate opencode template

- **WHEN** user runs `openspec-orchestrator init --template opencode`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with OpenCode template
- **AND** commands use `opencode run` pattern

#### Scenario: Generate claude template explicitly

- **WHEN** user runs `openspec-orchestrator init --template claude`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Claude Code template
- **AND** commands use `claude --dangerously-skip-permissions -p` pattern

#### Scenario: Generate codex template

- **WHEN** user runs `openspec-orchestrator init --template codex`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Codex template
- **AND** commands use `codex` pattern

#### Scenario: Config file already exists without force flag

- **WHEN** user runs `openspec-orchestrator init`
- **AND** `.openspec-orchestrator.jsonc` already exists in the current directory
- **THEN** the command exits with an error
- **AND** an error message indicates the file already exists
- **AND** suggests using `--force` to overwrite

#### Scenario: Overwrite existing config with force flag

- **WHEN** user runs `openspec-orchestrator init --force`
- **AND** `.openspec-orchestrator.jsonc` already exists in the current directory
- **THEN** the existing file is overwritten with the new template
- **AND** a success message is displayed

#### Scenario: Invalid template name

- **WHEN** user runs `openspec-orchestrator init --template invalid`
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
- **THEN** the footer displays "Press F5 to start processing"

### Requirement: Running Footer Progress Bar Display

The running mode footer SHALL display a progress bar for overall processing progress.

#### Scenario: Progress bar display during execution

- **WHEN** TUI is in running mode
- **THEN** the footer displays a progress bar for all tasks in the queue
- **AND** the progress bar is calculated based on completed tasks / total tasks
- **AND** the percentage is displayed numerically

#### Scenario: Progress bar calculation method

- **WHEN** displaying the progress bar
- **THEN** total tasks is the sum of `total_tasks` for all target changes (Queued, Processing, Completed, Archived)
- **AND** completed tasks is the sum of `completed_tasks` for all target changes
- **AND** progress rate is calculated as `completed_tasks / total_tasks * 100`
- **AND** NotQueued and Error state changes are NOT included in progress calculation

#### Scenario: Completed task progress retention

- **WHEN** a change transitions to Completed or Archived state
- **THEN** its task progress continues to be included in progress bar calculation
- **AND** progress percentage does not decrease (monotonically increasing)

#### Scenario: When task count is 0

- **WHEN** displaying the progress bar
- **AND** total task count is 0
- **THEN** the progress bar displays as 0%

### Requirement: Processing Item Spinner Animation

The TUI SHALL display an animated spinner next to items with `Processing` status in running mode.

#### Scenario: Spinner display for processing items
- **WHEN** TUI is in running mode
- **AND** an item has `QueueStatus::Processing`
- **THEN** an animated spinner character is displayed before the progress percentage
- **AND** the spinner cycles through Braille dot characters (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
- **AND** the display format is "⠋ [XX%]" where ⠋ is the current spinner character

#### Scenario: Spinner animation timing
- **WHEN** TUI is rendering in running mode
- **THEN** the spinner character advances to the next frame approximately every 100ms
- **AND** the spinner cycles continuously until processing completes

#### Scenario: Spinner not shown for non-processing items
- **WHEN** TUI is in running mode
- **AND** an item has status other than `Processing` (Queued, Completed, Error)
- **THEN** no spinner is displayed for that item

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

The CLI SHALL support a `--version` flag to display the application version.

#### Scenario: Display version with --version flag
- **WHEN** user runs `openspec-orchestrator --version`
- **THEN** the application version from Cargo.toml is displayed
- **AND** the program exits with code 0

#### Scenario: Display version with -V short flag
- **WHEN** user runs `openspec-orchestrator -V`
- **THEN** the application version is displayed (same as `--version`)

### Requirement: TUI Header Version Display

The TUI header SHALL display the application version in both selection and running modes.

#### Scenario: Version in selection mode header
- **WHEN** TUI is in selection mode
- **THEN** the header displays the application version (e.g., "v0.1.0")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

#### Scenario: Version in running mode header
- **WHEN** TUI is in running mode
- **THEN** the header displays the application version (e.g., "v0.1.0")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

### Requirement: Terminal Status Task Count Display

The TUI running mode SHALL display terminal states with status-only text and task counts in a separate column, avoiding redundant display.

#### Scenario: Completed state display format
- **WHEN** a change is in `completed` status in running mode
- **THEN** the status text SHALL be displayed as `[completed]` (without task count)
- **AND** the status is displayed in green color
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Archived state display format
- **WHEN** a change is in `archived` status in running mode
- **THEN** the status text SHALL be displayed as `[archived]` (without task count)
- **AND** the status is displayed in blue color
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Error state display format
- **WHEN** a change is in `error` status in running mode
- **THEN** the status text SHALL be displayed as `[error]` (without task count)
- **AND** the status is displayed in red color
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Processing state keeps progress percentage with task count
- **WHEN** a change is in `processing` status in running mode
- **THEN** the status text SHALL continue to display progress percentage as `⠋ [ XX%]`
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

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

The TUI SHALL track archived changes reliably and report accurate final status.

#### Scenario: All changes archived successfully
- **WHEN** all queued changes have been processed and archived
- **THEN** the final verification reports "All processed changes have been archived"
- **AND** no unarchived warnings are displayed

#### Scenario: Archive failure handling
- **WHEN** an archive command fails for a change
- **THEN** the change is marked as errored
- **AND** the error is logged with details
- **AND** the change is not removed from tracking until explicitly handled

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

### Requirement: approve Subcommand

The CLI SHALL provide an `approve` subcommand to manage change approval status.

#### Scenario: Approve a change with set action

- **WHEN** user runs `openspec-orchestrator approve set {change_id}`
- **AND** the change directory `openspec/changes/{change_id}/` exists
- **THEN** an `approved` file is created in the change directory
- **AND** the file contains MD5 checksums of all `.md` files (except `tasks.md`)
- **AND** a success message is displayed

#### Scenario: Approve a change that doesn't exist

- **WHEN** user runs `openspec-orchestrator approve set {change_id}`
- **AND** the change directory does not exist
- **THEN** an error message is displayed
- **AND** exit code is non-zero

#### Scenario: Unapprove a change with unset action

- **WHEN** user runs `openspec-orchestrator approve unset {change_id}`
- **AND** the `approved` file exists
- **THEN** the `approved` file is deleted
- **AND** a success message is displayed

#### Scenario: Unapprove a change that is not approved

- **WHEN** user runs `openspec-orchestrator approve unset {change_id}`
- **AND** the `approved` file does not exist
- **THEN** a message indicates the change was not approved
- **AND** exit code is zero (no-op)

#### Scenario: Check approval status

- **WHEN** user runs `openspec-orchestrator approve status {change_id}`
- **THEN** the approval status is displayed
- **AND** if approved, shows "approved" with file count
- **AND** if not approved, shows reason (file missing, hash mismatch, etc.)

### Requirement: TUI Approval Toggle

The TUI SHALL allow users to toggle approval status using the `@` key, with different auto-queue behavior based on orchestrator state.

#### Scenario: Approve unapproved change in Running mode (approve only)

- **WHEN** TUI is in Running mode (orchestrator actively processing)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved but NOT queued (`[@]`)
- **AND** checkbox transitions from `[ ]` to `[@]`
- **AND** log message indicates approval only

#### Scenario: Approve unapproved change in Select mode adds to queue automatically

- **WHEN** TUI is in Select mode (orchestrator stopped)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved AND queued (`[x]`)
- **AND** checkbox transitions directly from `[ ]` to `[x]`
- **AND** log message indicates both approval and queue addition

#### Scenario: Approve unapproved change in Completed mode adds to queue automatically

- **WHEN** TUI is in Completed mode (orchestrator stopped, all queued changes done)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved AND queued (`[x]`)
- **AND** checkbox transitions directly from `[ ]` to `[x]`
- **AND** log message indicates both approval and queue addition

#### Scenario: Unapprove approved-but-not-queued change

- **WHEN** TUI is in any mode (Select, Running, or Completed)
- **AND** user presses `@` key on an approved but not queued change (`[@]`)
- **THEN** the change becomes unapproved (`[ ]`)
- **AND** checkbox transitions from `[@]` to `[ ]`

#### Scenario: Unapprove queued change removes from queue

- **WHEN** TUI is in any mode (Select, Running, or Completed)
- **AND** user presses `@` key on a queued change (`[x]`) that is NOT processing
- **THEN** the change becomes unapproved AND removed from queue (`[ ]`)
- **AND** checkbox transitions from `[x]` to `[ ]`
- **AND** log message indicates both unapproval and queue removal

#### Scenario: Toggle approval blocked for processing change

- **WHEN** TUI is in Running mode
- **AND** user presses `@` key
- **AND** highlighted change is in `Processing` state
- **THEN** approval status is NOT changed
- **AND** a warning message is displayed: "Cannot change approval for processing change"

### Requirement: Auto-Queue Approved Changes on TUI Startup

The TUI SHALL automatically queue approved changes when starting in TUI mode.

#### Scenario: TUI startup with approved changes

- **WHEN** user starts the TUI
- **AND** one or more changes have valid `approved` files
- **THEN** those changes are automatically selected and queued
- **AND** a log message indicates "Auto-queued N approved changes"

#### Scenario: TUI startup with no approved changes

- **WHEN** user starts the TUI
- **AND** no changes have valid `approved` files
- **THEN** no changes are automatically queued
- **AND** the user can manually select and approve changes

### Requirement: Unapproved Changes Cannot Be Queued

The system SHALL prevent unapproved changes from being added to the execution queue.

#### Scenario: Attempt to queue unapproved change in TUI

- **WHEN** TUI is in selection mode
- **AND** user presses Space to select an unapproved change
- **THEN** the change can be selected for viewing
- **AND** pressing F5 with only unapproved changes selected shows warning
- **AND** the warning suggests approving changes first

#### Scenario: CLI run with unapproved change

- **WHEN** user runs `openspec-orchestrator run --change {change_id}`
- **AND** the change is not approved
- **THEN** a warning message is displayed
- **AND** the change is NOT added to the queue
- **AND** processing continues with any remaining approved changes

#### Scenario: CLI run with mixed approved/unapproved changes

- **WHEN** user runs `openspec-orchestrator run --change a,b,c`
- **AND** change `a` is approved, `b` is not approved, `c` is approved
- **THEN** warning is displayed for change `b`
- **AND** only changes `a` and `c` are processed

### Requirement: Log Entry Limit

The TUI SHALL maintain a maximum limit on stored log entries to prevent unbounded memory growth.

#### Scenario: Log entry limit enforcement
- **WHEN** a new log entry is added
- **AND** the total log count exceeds 1000 entries
- **THEN** the oldest log entry is removed
- **AND** scroll offset is adjusted if necessary to prevent display issues

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

The orchestrator MUST capture the agent's final summary message from each apply attempt and include it in subsequent apply prompts for the same change.

#### Scenario: First apply attempt has no history

- **WHEN** the orchestrator executes apply for a change for the first time
- **THEN** the prompt contains only the base apply_prompt from configuration
- **AND** no `<last_apply>` tags are included

#### Scenario: Second apply includes previous attempt summary

- **WHEN** the orchestrator executes apply for a change for the second time
- **AND** the first attempt returned a summary message from the agent
- **THEN** the prompt contains the base apply_prompt
- **AND** the prompt contains a `<last_apply attempt="1">` block
- **AND** the block contains the agent's summary message from the first attempt

#### Scenario: Multiple previous attempts are included

- **WHEN** the orchestrator executes apply for a change for the third time
- **THEN** the prompt contains `<last_apply attempt="1">` and `<last_apply attempt="2">` blocks
- **AND** blocks are ordered by attempt number (oldest first)
- **AND** each block contains the agent's summary message from that attempt

#### Scenario: History is cleared on archive

- **WHEN** a change is successfully archived
- **THEN** the apply history for that change is cleared from memory
- **AND** subsequent apply attempts for the same change_id (if unarchived) start fresh

### Requirement: Apply History Context Format

The apply history context MUST be formatted as XML-like tags containing the agent's summary message.

#### Scenario: Context format structure

- **GIVEN** a previous apply attempt where the agent returned the summary:
  "Implemented task 1.1 and 1.2. Found issue with type conversion in auth.rs:42 that needs fixing."
- **WHEN** the context is formatted for the next prompt
- **THEN** the output is:
  ```
  <last_apply attempt="1">
  Implemented task 1.1 and 1.2. Found issue with type conversion in auth.rs:42 that needs fixing.
  </last_apply>
  ```

#### Scenario: Context appended to base prompt

- **GIVEN** base apply_prompt is "Delete out-of-scope tasks"
- **AND** there is one previous attempt with agent summary "Task 1.1 completed."
- **WHEN** the full prompt is built
- **THEN** the prompt format is:
  ```
  Delete out-of-scope tasks

  <last_apply attempt="1">
  Task 1.1 completed.
  </last_apply>
  ```

#### Scenario: Agent summary captured from apply response

- **WHEN** the openspec:apply skill completes execution
- **THEN** the agent returns a summary message describing work done
- **AND** the orchestrator captures this summary message for history

### Requirement: TUI Stop Processing with Escape Key
TUIはEsc二度押しによる強制停止時、現在のエージェントプロセスとその子プロセスを確実に終了しなければならない（SHALL）。

#### Scenario: 強制停止で子プロセスが残らない
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** 現在のエージェントプロセスとその子プロセスが終了する
- **AND** 終了待機がタイムアウトした場合でも、追加の終了処理が行われる
- **AND** ログに「Force stopped - process terminated」が表示される
- **AND** 変更の状態はQueuedに戻る

### Requirement: TUI Stopped Mode

The TUI SHALL provide a Stopped mode where users can review progress and manage the queue before resuming. Task completion in Stopped mode SHALL NOT automatically add changes to the queue.

#### Scenario: Stopped mode display

- **WHEN** TUI is in Stopped mode
- **THEN** header status displays "Stopped" in gray color
- **AND** the change list remains visible with current statuses
- **AND** completed changes show "[completed]" or "[archived]"
- **AND** remaining queued changes show "queued"

#### Scenario: Queue management in Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a queued change
- **THEN** the change is removed from queue (becomes not queued)

#### Scenario: Queue addition in Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-queued change
- **THEN** the change is added to the queue

#### Scenario: Task completion in Stopped mode does not auto-queue

- **WHEN** TUI is in Stopped mode
- **AND** a change's tasks are updated (e.g., all tasks marked complete)
- **THEN** the change queue_status SHALL remain unchanged
- **AND** the change SHALL NOT be automatically added to the queue
- **AND** the change SHALL only be queued when user explicitly presses Space key

#### Scenario: Resume processing from Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** one or more changes are queued
- **AND** user presses F5
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes with the queued changes
- **AND** log displays "Resuming processing..."

#### Scenario: Resume with empty queue shows warning

- **WHEN** TUI is in Stopped mode
- **AND** no changes are queued
- **AND** user presses F5
- **THEN** a warning message is displayed
- **AND** the TUI remains in Stopped mode

### Requirement: TUI Help Text for Stop

The TUI help text SHALL include stop key binding information.

#### Scenario: Running mode help text

- **WHEN** TUI is in Running mode
- **THEN** help text includes "Esc: stop"
- **AND** help text continues to show "q: quit"

#### Scenario: Stopping mode help text

- **WHEN** TUI is in Stopping mode
- **THEN** help text includes "Esc: force stop"
- **AND** help text shows "Waiting for current process..."

#### Scenario: Stopped mode help text

- **WHEN** TUI is in Stopped mode
- **THEN** help text includes "F5: resume"
- **AND** help text includes "Space: toggle queue"
- **AND** help text includes "q: quit"

### Requirement: Interrupted Change Handling

Changes interrupted by force stop SHALL be handled gracefully.

#### Scenario: Force-stopped change returns to queued

- **WHEN** a change is being processed
- **AND** user force stops with second Esc press
- **THEN** the change status becomes "queued" (not error)
- **AND** the change can be re-processed on resume
- **AND** no error message is displayed for the interruption

#### Scenario: Partial progress preserved

- **WHEN** a change had some tasks completed before force stop
- **THEN** the completed tasks remain completed
- **AND** the tasks.md file reflects actual progress
- **AND** resuming continues from the partial state

### Requirement: Parallel Execution Mode Flag

The CLI SHALL support a `--parallel` flag to enable parallel change execution using git worktrees. Parallel mode is OFF by default.

#### Scenario: Enable parallel mode via CLI flag
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** a `.git` directory exists
- **THEN** the orchestrator enters parallel execution mode
- **AND** changes are analyzed for parallelization opportunities

#### Scenario: Parallel mode disabled by default
- **WHEN** user runs `openspec-orchestrator run` without `--parallel` flag
- **THEN** the orchestrator uses sequential execution mode
- **AND** no parallelization analysis is performed

#### Scenario: Parallel mode requires git directory
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** no `.git` directory exists
- **THEN** the command exits with error code 1
- **AND** an error message indicates git repository is required for parallel mode

#### Scenario: Parallel mode with max concurrent limit
- **WHEN** user runs `openspec-orchestrator run --parallel --max-concurrent 4`
- **THEN** at most 4 workspaces are created simultaneously
- **AND** additional changes wait until a workspace becomes available

### Requirement: Parallel Mode TUI Display

The TUI SHALL display parallel execution progress when in parallel mode.

#### Scenario: Display parallel groups
- **WHEN** TUI is in running mode with parallel execution
- **THEN** changes are grouped by their parallel group assignment
- **AND** each group is visually distinguished

#### Scenario: Display workspace status
- **WHEN** changes are being processed in parallel
- **THEN** each change shows its workspace status (creating, running, completed, failed)
- **AND** multiple spinners can be active simultaneously

#### Scenario: Display merge progress
- **WHEN** a parallel group completes and merge begins
- **THEN** a merge progress indicator is displayed
- **AND** the merge result (success/conflict) is shown

### Requirement: Parallel Mode Dry Run

The CLI SHALL support `--dry-run` to preview parallelization groups without execution.

#### Scenario: Preview parallelization groups
- **WHEN** user runs `openspec-orchestrator run --parallel --dry-run`
- **THEN** the analyzer determines parallelization groups
- **AND** the groups are displayed without executing any changes
- **AND** no workspaces are created

### Requirement: VCS Backend Selection Flag

CLI SHALL allow explicit VCS backend selection via `--vcs` flag.

#### Scenario: Default auto detection

- **WHEN** `--parallel` flag is specified
- **AND** `--vcs` flag is not specified
- **THEN** VCS backend is auto-detected
- **AND** Git backend is selected when a `.git` directory exists

#### Scenario: Explicit git selection

- **WHEN** `openspec-orchestrator run --parallel --vcs git` is executed
- **THEN** Git backend is used
- **AND** an error is displayed if Git is not available

#### Scenario: Explicit auto selection

- **WHEN** `openspec-orchestrator run --parallel --vcs auto` is executed
- **THEN** VCS backend is auto-detected
- **AND** Git backend is selected when a `.git` directory exists

#### Scenario: Invalid VCS value

- **WHEN** `openspec-orchestrator run --parallel --vcs invalid` is executed
- **THEN** error message "Invalid VCS backend: invalid. Valid options: auto, git" is displayed
- **AND** exit code is non-zero

#### Scenario: --vcs without --parallel

- **WHEN** `openspec-orchestrator run --vcs git` is executed
- **AND** `--parallel` flag is not specified
- **THEN** `--vcs` option is ignored
- **AND** normal sequential execution proceeds

### Requirement: Git Uncommitted Changes Error Message

When uncommitted changes exist with Git backend, CLI SHALL display a detailed error message.

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
- **AND** files in `.gitignore` are excluded

### Requirement: Archived 状態の checkbox 表示

TUI は archived 状態の change の checkbox をグレー色で表示しなければならない（SHALL）。

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

The CLI SHALL support flags to enable and configure web-based monitoring.

#### Scenario: Enable web monitoring
- **WHEN** user runs with `--web` flag
- **THEN** HTTP server starts for web monitoring
- **AND** server binds to default port 8080 on 127.0.0.1

#### Scenario: Configure web port
- **WHEN** user runs with `--web --web-port 3000`
- **THEN** HTTP server starts on port 3000 instead of default

#### Scenario: Configure bind address
- **WHEN** user runs with `--web --web-bind 0.0.0.0`
- **THEN** HTTP server accepts connections from any network interface
- **AND** warning is logged about exposing server to network

#### Scenario: Web flags without --web
- **WHEN** user runs with `--web-port 3000` but without `--web` flag
- **THEN** HTTP server does not start
- **AND** web-port flag is ignored

#### Scenario: Invalid port number
- **WHEN** user runs with `--web --web-port 99999`
- **THEN** error message is displayed about invalid port range
- **AND** orchestrator exits with non-zero status

#### Scenario: Web monitoring in TUI mode
- **WHEN** user runs TUI mode with `--web` flag
- **THEN** HTTP server starts in background
- **AND** TUI displays message indicating web server is running
- **AND** TUI shows web server URL (e.g., "Web monitoring: http://127.0.0.1:8080")

#### Scenario: Web monitoring in run mode
- **WHEN** user runs `openspec-orchestrator run --web`
- **THEN** HTTP server starts before orchestration begins
- **AND** server URL is logged to console
- **AND** orchestration proceeds normally

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

The CLI SHALL detect whether the current directory is a git-managed repository by checking for the `.git` directory.

#### Scenario: git repository detected
- **WHEN** a `.git` directory exists in the current working directory
- **THEN** git worktree parallel features are available

#### Scenario: git repository not detected
- **WHEN** no `.git` directory exists in the current working directory
- **AND** user runs `openspec-orchestrator run --parallel`
- **THEN** the command exits with a non-zero exit code
- **AND** an error message is displayed: "Error: --parallel requires a git repository (.git directory not found)"

### Requirement: Serial Apply Iteration WIP Commits
逐次（非parallel）applyループでは、各イテレーション終了後に作業内容をWIPコミットとして保存しなければならない（MUST）。apply成功・失敗や進捗増加の有無に関わらず、最新状態をスナップショットとして残さなければならない（MUST）。

WIPコミットメッセージは `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})` の形式としなければならない（MUST）。Gitリポジトリで実行中の場合、`git add -A` と `git commit --allow-empty` 相当の操作で新規WIPコミットを作成しなければならない（MUST）。既存WIPコミットの `--amend` を使用してはならない（MUST NOT）。

#### Scenario: WIP created after successful apply iteration
- Given: 逐次applyループが実行中である
- When: applyコマンドが正常に完了しイテレーションが終了する
- Then: WIPスナップショットが新規コミットとして作成される

#### Scenario: WIP created after failed apply iteration
- Given: 逐次applyループが実行中である
- When: applyコマンドが失敗してイテレーションが終了する
- Then: 失敗時点の作業内容がWIPスナップショットとして保存される

#### Scenario: Snapshot created even when no progress made
- Given: applyコマンドが正常に完了したがタスク進捗が増加しなかった
- When: イテレーションが終了する
- Then: 最新の作業内容を反映したWIPスナップショットが作成される

#### Scenario: WIP message includes iteration index
- Given: WIPスナップショットを作成する
- When: コミットメッセージを設定する
- Then: メッセージに `apply#{iteration}` が含まれる

#### Scenario: Git backend uses allow-empty commit
- Given: Gitリポジトリで逐次applyループが実行中である
- When: WIPスナップショットを作成する
- Then: `git add -A` と `git commit --allow-empty` 相当の操作で新規コミットが作成される
