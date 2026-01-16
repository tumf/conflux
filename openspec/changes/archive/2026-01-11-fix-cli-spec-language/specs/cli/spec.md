## MODIFIED Requirements

### Requirement: Subcommand Structure

CLI SHALL have a subcommand structure that supports future command extensions.

#### Scenario: Run without subcommand

- **WHEN** user runs `cflx` without arguments
- **THEN** the interactive TUI is launched
- **AND** the change list is displayed in selection mode

#### Scenario: Run with unknown subcommand

- **WHEN** user runs with a non-existent subcommand
- **THEN** an error message with available subcommands is displayed

### Requirement: run Subcommand

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.

#### Scenario: Run with specific change

- **WHEN** user runs `cflx run --change <id>`
- **THEN** only the specified change is processed
- **AND** the snapshot log shows only the specified change

#### Scenario: Run with comma-separated changes

- **WHEN** user runs `cflx run --change a,b,c`
- **THEN** only changes `a`, `b`, `c` are processed
- **AND** the snapshot log shows only `a`, `b`, `c`

#### Scenario: Run with non-existent change

- **WHEN** user runs `cflx run --change nonexistent`
- **AND** no change named `nonexistent` exists
- **THEN** a warning message "Specified change 'nonexistent' not found, skipping" is displayed
- **AND** exits with "No changes found"

#### Scenario: Run with mixed valid and invalid changes

- **WHEN** user runs `cflx run --change a,nonexistent,c`
- **AND** `a` and `c` exist but `nonexistent` does not
- **THEN** a warning message "Specified change 'nonexistent' not found, skipping" is displayed
- **AND** only `a` and `c` are processed
- **AND** the snapshot log shows only `a` and `c`

### Requirement: Default TUI Launch

When launched without a subcommand, the interactive TUI SHALL be displayed.

#### Scenario: Launch without subcommand

- **WHEN** user runs `cflx` without arguments
- **THEN** the interactive TUI is launched
- **AND** the change list is displayed in selection mode

#### Scenario: Launch with run subcommand (backward compatibility)

- **WHEN** user runs `cflx run`
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

### Requirement: Dynamic Execution Queue

In running mode, unselected changes can be added to the queue, and queued changes can be removed. Added changes SHALL be processed by the orchestrator.

#### Scenario: Queue addition during execution

- **WHEN** TUI is in running mode
- **AND** user moves cursor to an unselected change (NotQueued) and presses Space key
- **THEN** the change is added to the execution queue
- **AND** display updates from "not queued" to "queued"
- **AND** the change ID is pushed to the shared queue

#### Scenario: Remove queued change

- **WHEN** TUI is in running mode
- **AND** user moves cursor to a queued change (Queued) and presses Space key
- **THEN** the change is removed from the queue
- **AND** display updates from "queued" to "not queued"
- **AND** the selection is cleared

#### Scenario: Processing order after queue addition

- **WHEN** a change is dynamically added to the queue
- **THEN** it is processed after the currently processing change completes
- **AND** the order of existing queued changes is unchanged

#### Scenario: Processing change cannot be modified

- **WHEN** a change is Processing
- **THEN** its selection state cannot be changed
- **AND** pressing Space key has no effect

#### Scenario: Archiving change cannot be modified

- **WHEN** a change is being archived
- **THEN** its selection state cannot be changed
- **AND** pressing Space key has no effect

#### Scenario: Dynamic queue addition in Waiting state

- **WHEN** TUI is in running mode showing "Waiting..."
- **AND** no change is currently processing
- **AND** user moves cursor to an unselected change (NotQueued) and presses Space key
- **THEN** the change is added to the execution queue
- **AND** the orchestrator detects and starts processing the change
- **AND** log displays "Processing dynamically added: <change-id>"

#### Scenario: Dynamically added change processing completion

- **WHEN** processing of a dynamically added change completes
- **THEN** the change status updates to "completed" or "archived"
- **AND** remaining dynamic queue items continue processing
- **AND** "AllCompleted" event is sent when both initial and dynamic queues are empty

#### Scenario: Prevent duplicate addition

- **WHEN** attempting to add a change that already exists in the queue
- **THEN** the addition is ignored
- **AND** a warning log is displayed

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

### Requirement: VCS Backend Selection Flag

CLI SHALL allow explicit VCS backend selection via `--vcs` flag.

#### Scenario: Default auto detection

- **WHEN** `--parallel` flag is specified
- **AND** `--vcs` flag is not specified
- **THEN** VCS backend is auto-detected (jj preferred)

#### Scenario: Explicit jj selection

- **WHEN** `cflx run --parallel --vcs jj` is executed
- **THEN** jj backend is used
- **AND** an error is displayed if jj is not available

#### Scenario: Explicit git selection

- **WHEN** `cflx run --parallel --vcs git` is executed
- **THEN** Git backend is used
- **AND** Git is used even if jj exists
- **AND** an error is displayed if Git is not available

#### Scenario: Explicit auto selection

- **WHEN** `cflx run --parallel --vcs auto` is executed
- **THEN** VCS backend is auto-detected
- **AND** jj is preferred, Git is used if jj is not available

#### Scenario: Invalid VCS value

- **WHEN** `cflx run --parallel --vcs invalid` is executed
- **THEN** error message "Invalid VCS backend: invalid. Valid options: auto, jj, git" is displayed
- **AND** exit code is non-zero

#### Scenario: --vcs without --parallel

- **WHEN** `cflx run --vcs git` is executed
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
