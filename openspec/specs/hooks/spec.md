# hooks Specification

## Purpose
Defines the lifecycle hook system including available hooks, context variables, and execution order.
## Requirements
### Requirement: on_queue_add hook

The orchestrator SHALL execute `on_queue_add` hook when a user dynamically adds a change to the queue (via Space key in TUI).

#### Scenario: User adds change to queue in TUI

- **GIVEN** `hooks.on_queue_add` is set to `echo 'Added {change_id}'`
- **AND** TUI is in Running or Stopped mode
- **WHEN** user presses Space on an unapproved/not-queued change
- **THEN** `on_queue_add` is executed with the change ID

#### Scenario: on_queue_add not called for initial queue

- **GIVEN** `hooks.on_queue_add` is configured
- **AND** user selects 3 changes before starting orchestration
- **WHEN** orchestration starts
- **THEN** `on_queue_add` is NOT called (only for dynamic additions)

### Requirement: on_queue_remove hook

The orchestrator SHALL execute `on_queue_remove` hook when a user dynamically removes a change from the queue (via Space key in TUI).

#### Scenario: User removes change from queue in TUI

- **GIVEN** `hooks.on_queue_remove` is set to `echo 'Removed {change_id}'`
- **AND** TUI is in Running or Stopped mode
- **WHEN** user presses Space on a queued change
- **THEN** `on_queue_remove` is executed with the change ID

### Requirement: on_approve hook

The orchestrator SHALL execute `on_approve` hook when a user approves a change (via @ key in TUI).

#### Scenario: User approves a change in TUI

- **GIVEN** `hooks.on_approve` is set to `echo 'Approved {change_id}'`
- **WHEN** user presses @ on an unapproved change
- **THEN** `on_approve` is executed with the change ID

#### Scenario: on_approve receives change context

- **GIVEN** `hooks.on_approve` is configured
- **AND** change `my-change` has 2/5 tasks completed
- **WHEN** user approves `my-change`
- **THEN** `on_approve` receives `{change_id}=my-change`
- **AND** `{completed_tasks}=2` and `{total_tasks}=5` are available

### Requirement: on_unapprove hook

The orchestrator SHALL execute `on_unapprove` hook when a user removes approval from a change (via @ key in TUI).

#### Scenario: User unapproves a change in TUI

- **GIVEN** `hooks.on_unapprove` is set to `echo 'Unapproved {change_id}'`
- **WHEN** user presses @ on an approved change
- **THEN** `on_unapprove` is executed with the change ID

#### Scenario: on_unapprove with queued change

- **GIVEN** `hooks.on_unapprove` is configured
- **AND** change `my-change` is approved and queued
- **WHEN** user presses @ to unapprove
- **THEN** `on_unapprove` is executed
- **AND** the change is also removed from queue (on_queue_remove is NOT called separately)

### Requirement: on_change_start hook

The orchestrator SHALL execute `on_change_start` hook when starting to process a new change.

The hook SHALL be called exactly once per change, before the first `pre_apply` for that change.

#### Scenario: Basic on_change_start execution

- **GIVEN** `hooks.on_change_start` is set to `echo 'Starting {change_id}'`
- **AND** changes `change-a` and `change-b` exist
- **WHEN** the orchestrator processes both changes
- **THEN** `on_change_start` is called once for `change-a`
- **AND** `on_change_start` is called once for `change-b`

#### Scenario: on_change_start with jj integration

- **GIVEN** `hooks.on_change_start` is set to `jj new -m 'changeset: {change_id}'`
- **WHEN** the orchestrator starts processing change `add-feature`
- **THEN** a new jj change is created with message `changeset: add-feature`

#### Scenario: on_change_start has change_id available

- **GIVEN** `hooks.on_change_start` is set to `echo $OPENSPEC_CHANGE_ID`
- **WHEN** the orchestrator starts processing change `my-change`
- **THEN** the hook receives `OPENSPEC_CHANGE_ID=my-change`
- **AND** `{change_id}` placeholder expands to `my-change`

### Requirement: on_change_end hook

The orchestrator SHALL execute `on_change_end` hook after a change has been fully processed (archived).

#### Scenario: Basic on_change_end execution

- **GIVEN** `hooks.on_change_end` is set to `echo 'Finished {change_id}'`
- **AND** change `change-a` reaches 100% completion and is archived
- **WHEN** the archive completes successfully
- **THEN** `on_change_end` is called with `{change_id}=change-a`

#### Scenario: on_change_end not called on error

- **GIVEN** `hooks.on_change_end` is configured
- **AND** apply fails for change `change-a`
- **WHEN** processing stops due to error
- **THEN** `on_change_end` is NOT called for `change-a`

#### Scenario: on_change_end tracks progress

- **GIVEN** `hooks.on_change_end` is set to `echo '{changes_processed}/{total_changes}'`
- **AND** 3 changes exist
- **WHEN** the first change is archived
- **THEN** the hook outputs `1/3`

### Requirement: Hook context variables

The orchestrator SHALL provide the following context to all hooks via environment variables and placeholders:

| Variable / Placeholder | Description | Hooks |
|------------------------|-------------|-------|
| OPENSPEC_CHANGE_ID / {change_id} | Current change ID | All except on_start/on_finish |
| OPENSPEC_CHANGES_PROCESSED / {changes_processed} | Number of archived changes | All |
| OPENSPEC_TOTAL_CHANGES / {total_changes} | Initial queue size | All |
| OPENSPEC_REMAINING_CHANGES / {remaining_changes} | Remaining changes in queue | All |
| OPENSPEC_COMPLETED_TASKS / {completed_tasks} | Completed tasks in change | Change-specific |
| OPENSPEC_TOTAL_TASKS / {total_tasks} | Total tasks in change | Change-specific |
| OPENSPEC_APPLY_COUNT / {apply_count} | Times this change was applied | Change-specific |
| OPENSPEC_STATUS / {status} | Finish status | on_finish |
| OPENSPEC_ERROR / {error} | Error message | on_error |

#### Scenario: Environment variables match placeholders

- **GIVEN** `hooks.pre_apply` is set to `echo $OPENSPEC_CHANGE_ID`
- **AND** change `my-change` is being processed
- **WHEN** pre_apply hook runs
- **THEN** `OPENSPEC_CHANGE_ID` environment variable equals `my-change`

#### Scenario: New variables are available

- **GIVEN** `hooks.on_change_end` is set to `echo $OPENSPEC_CHANGES_PROCESSED $OPENSPEC_REMAINING_CHANGES`
- **AND** 3 total changes exist
- **WHEN** first change is archived
- **THEN** `OPENSPEC_CHANGES_PROCESSED=1` and `OPENSPEC_REMAINING_CHANGES=2`

### Requirement: Placeholder availability per hook

Each hook SHALL have access to the following placeholders:

| Placeholder | on_start | on_change_start | pre_apply | post_apply | on_change_complete | pre_archive | post_archive | on_change_end | on_finish | on_error | on_queue_add | on_queue_remove | on_approve | on_unapprove |
|-------------|----------|-----------------|-----------|------------|-------------------|-------------|--------------|---------------|-----------|----------|--------------|-----------------|------------|--------------|
| {change_id} | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅* | ✅ | ✅ | ✅ | ✅ |
| {changes_processed} | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| {total_changes} | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| {remaining_changes} | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| {completed_tasks} | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅* | ✅ | ✅ | ✅ | ✅ |
| {total_tasks} | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅* | ✅ | ✅ | ✅ | ✅ |
| {apply_count} | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅* | ❌ | ❌ | ❌ | ❌ |
| {status} | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| {error} | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |

*Note: on_error has change-related placeholders only when the error occurred during change processing.

#### Scenario: on_start has no change_id

- **GIVEN** `hooks.on_start` is set to `echo '{change_id}'`
- **WHEN** orchestration starts
- **THEN** `{change_id}` is NOT expanded (remains as literal string or empty)

#### Scenario: on_finish has status but no change_id

- **GIVEN** `hooks.on_finish` is set to `echo 'Status: {status}, Changes: {changes_processed}/{total_changes}'`
- **WHEN** orchestration completes normally
- **THEN** output is `Status: completed, Changes: 3/3` (example)

#### Scenario: User interaction hooks have change context

- **GIVEN** `hooks.on_approve` is set to `echo '{change_id}: {completed_tasks}/{total_tasks}'`
- **AND** change `my-change` has 2/5 tasks
- **WHEN** user approves `my-change`
- **THEN** output is `my-change: 2/5`

#### Scenario: apply_count increments with each apply

- **GIVEN** `hooks.post_apply` is set to `echo 'Apply #{apply_count}'`
- **AND** change `my-change` requires 3 applies to complete
- **WHEN** the orchestrator applies `my-change` three times
- **THEN** post_apply outputs `Apply #1`, `Apply #2`, `Apply #3`

#### Scenario: changes_processed updates after archive

- **GIVEN** `hooks.on_change_start` is set to `echo '{changes_processed} done'`
- **AND** 3 changes exist, all starting at 0%
- **WHEN** processing starts
- **THEN** first on_change_start outputs `0 done`
- **AND** after first change archives, second on_change_start outputs `1 done`

### Requirement: Hook execution order

The orchestrator SHALL execute hooks in the following order for each change:

1. `on_change_start` (once per change)
2. `pre_apply` → [apply] → `post_apply` (repeated until complete)
3. `on_change_complete` (when tasks reach 100%)
4. `pre_archive` → [archive] → `post_archive`
5. `on_change_end` (once per change)

Global hooks:
- `on_start`: Before any change processing
- `on_finish`: After all changes processed or on stop
- `on_error`: When an error occurs

#### Scenario: Full lifecycle for one change

- **GIVEN** all hooks are configured
- **AND** change `my-change` has 2 tasks, starts at 0%
- **WHEN** the orchestrator processes `my-change` (requires 2 applies)
- **THEN** hooks are called in order:
  1. on_start
  2. on_change_start (change_id=my-change)
  3. pre_apply (apply_count=1)
  4. post_apply (apply_count=1)
  5. pre_apply (apply_count=2)
  6. post_apply (apply_count=2)
  7. on_change_complete
  8. pre_archive
  9. post_archive
  10. on_change_end
  11. on_finish

#### Scenario: Change with 100% from start (no apply needed)

- **GIVEN** change `complete-change` has all tasks already done
- **WHEN** the orchestrator processes it
- **THEN** hooks are called:
  1. on_change_start
  2. on_change_complete
  3. pre_archive
  4. post_archive
  5. on_change_end

### Requirement: TUI and CLI hook parity

The orchestrator SHALL execute the same hooks with the same context in both TUI mode and CLI (run) mode.

#### Scenario: TUI executes on_change_start

- **GIVEN** `hooks.on_change_start` is configured
- **WHEN** user starts orchestration in TUI mode
- **AND** processing begins on a change
- **THEN** `on_change_start` is executed (same as CLI mode)

#### Scenario: TUI executes on_change_end

- **GIVEN** `hooks.on_change_end` is configured
- **WHEN** a change is archived in TUI mode
- **THEN** `on_change_end` is executed (same as CLI mode)

### Requirement: Hook configuration format

Hook configuration SHALL support both simple string form and detailed object form.

#### Scenario: Simple string hook

- **GIVEN** config contains:
  ```jsonc
  {
    "hooks": {
      "on_change_start": "jj new -m '{change_id}'"
    }
  }
  ```
- **WHEN** orchestrator loads the config
- **THEN** the hook is registered with default timeout (60s) and continue_on_failure (true)

#### Scenario: Detailed hook configuration

- **GIVEN** config contains:
  ```jsonc
  {
    "hooks": {
      "on_change_start": {
        "command": "jj new -m '{change_id}'",
        "timeout": 30,
        "continue_on_failure": false
      }
    }
  }
  ```
- **WHEN** orchestrator loads the config
- **THEN** the hook uses timeout=30s and continue_on_failure=false

### Requirement: Available hook types

The orchestrator SHALL support the following hook types:

**Run lifecycle:**
- `on_start`: Run loop started
- `on_finish`: Run loop finished
- `on_error`: Error occurred

**Change lifecycle:**
- `on_change_start`: Change processing started (once per change)
- `pre_apply`: Before apply execution
- `post_apply`: After successful apply
- `on_change_complete`: Change reached 100% task completion
- `pre_archive`: Before archive execution
- `post_archive`: After successful archive
- `on_change_end`: Change processing ended (after archive)

**User interaction (TUI only):**
- `on_queue_add`: User dynamically added a change to queue (Space key)
- `on_queue_remove`: User dynamically removed a change from queue (Space key)
- `on_approve`: User approved a change (@ key)
- `on_unapprove`: User removed approval from a change (@ key)

#### Scenario: Complete hook list in configuration

- **GIVEN** config contains all hook types
- **WHEN** orchestrator loads the config
- **THEN** all hooks are registered and executed at appropriate times

### Requirement: Configuration template hook examples

The `init` command templates SHALL include hook examples that demonstrate all available placeholders for each hook type.

Templates SHALL use simple string format (not object format with timeout/continue_on_failure).

#### Scenario: Claude template hook examples

- **WHEN** user runs `openspec-orchestrator init --template claude`
- **THEN** the hooks section contains commented examples for each hook type
- **AND** each example uses `echo` with all available placeholders for that hook
- **AND** no hook uses object format (timeout, continue_on_failure)

#### Scenario: on_start hook example shows available placeholders

- **GIVEN** the generated template
- **THEN** on_start example is `echo '[on_start] changes_processed={changes_processed} total={total_changes} remaining={remaining_changes}'`

#### Scenario: on_change_start hook example shows available placeholders

- **GIVEN** the generated template
- **THEN** on_change_start example is `echo '[on_change_start] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'`

#### Scenario: pre_apply hook example shows available placeholders

- **GIVEN** the generated template
- **THEN** pre_apply example is `echo '[pre_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'`

#### Scenario: on_finish hook example shows available placeholders

- **GIVEN** the generated template
- **THEN** on_finish example is `echo '[on_finish] status={status} processed={changes_processed}/{total_changes}'`

#### Scenario: on_error hook example shows available placeholders

- **GIVEN** the generated template
- **THEN** on_error example is `echo '[on_error] change={change_id} error={error}'`

#### Scenario: TUI-only hook examples

- **GIVEN** the generated template
- **THEN** on_queue_add example is `echo '[on_queue_add] change={change_id} tasks={completed_tasks}/{total_tasks}'`
- **AND** on_approve example is `echo '[on_approve] change={change_id} tasks={completed_tasks}/{total_tasks}'`

### Requirement: Parallel Mode Hook Context

parallel mode での hook 実行時、`HookContext` には workspace 固有の情報が含まれなければならない（SHALL）。

#### Scenario: Workspace path の提供

- **GIVEN** parallel mode で hook が実行される
- **WHEN** `HookContext` が構築される
- **THEN** 環境変数 `OPENSPEC_WORKSPACE_PATH` に workspace のパスが設定される

#### Scenario: Group 情報の提供

- **GIVEN** parallel mode で複数の change がグループとして処理されている
- **WHEN** hook が実行される
- **THEN** 環境変数 `OPENSPEC_GROUP_INDEX` に現在のグループインデックスが設定される
