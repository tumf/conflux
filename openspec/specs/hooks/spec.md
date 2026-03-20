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
各フックは以下のプレースホルダーにアクセスできなければならない（SHALL）。

| Placeholder | on_start | on_change_start | pre_apply | post_apply | on_change_complete | pre_archive | post_archive | on_change_end | on_merged | on_finish | on_error | on_queue_add | on_queue_remove | on_approve | on_unapprove |
|-------------|----------|-----------------|-----------|------------|-------------------|-------------|--------------|---------------|-----------|-----------|----------|--------------|-----------------|------------|--------------|
| {change_id} | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅* | ✅ | ✅ | ✅ | ✅ |
| {changes_processed} | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| {total_changes} | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| {remaining_changes} | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| {completed_tasks} | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅* | ✅ | ✅ | ✅ | ✅ |
| {total_tasks} | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅* | ✅ | ✅ | ✅ | ✅ |
| {apply_count} | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅* | ❌ | ❌ | ❌ | ❌ |
| {status} | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| {error} | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |

*Note: on_error はエラーが change 処理中に発生した場合のみ change 関連のプレースホルダーを持つ。

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

#### Scenario: on_merged has change context after merge
- **GIVEN** `hooks.on_merged` is set to `echo '{change_id} {completed_tasks}/{total_tasks}'`
- **AND** change `my-change` is merged to base branch
- **WHEN** `on_merged` is executed
- **THEN** `{change_id}` と進捗プレースホルダーが展開される

### Requirement: Hook execution order
オーケストレーターは、各 change に対して以下の順序でフックを実行しなければならない（SHALL）。

1. `on_change_start`（change ごとに 1 回）
2. `pre_apply` → [apply] → `post_apply`（完了まで繰り返す）
3. `on_change_complete`（タスク 100% 到達時）
4. `pre_archive` → [archive] → `post_archive`
5. `on_change_end`（archive 完了後）
6. `on_merged`（base branch へのマージ完了後）

Global hooks:
- `on_start`: 変更処理開始前
- `on_finish`: すべての change が処理完了または停止した後
- `on_error`: エラー発生時

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
  11. on_merged
  12. on_finish

#### Scenario: Change with 100% from start (no apply needed)
- **GIVEN** change `complete-change` has all tasks already done
- **WHEN** the orchestrator processes it
- **THEN** hooks are called:
  1. on_change_start
  2. on_change_complete
  3. pre_archive
  4. post_archive
  5. on_change_end
  6. on_merged

### Requirement: TUI and CLI hook parity

オーケストレーターは、TUI モードと CLI（run）モードで同一のフックを同一のコンテキストで実行しなければならない（SHALL）。

#### Scenario: CLI で hook 実行イベントを通知する
- **GIVEN** hooks が設定されており CLI（run）モードで change が処理中である
- **WHEN** apply/archive 中に hook が開始・完了する
- **THEN** hook 実行は parallel と同一のイベント通知で報告される
- **AND** hook 実行順序はライフサイクル定義に従う

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
オーケストレーターは以下の hook 種別をサポートしなければならない（SHALL）。

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
- `on_merged`: Change merged to base branch

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
`init` コマンドのテンプレートは、すべての hook 種別についてコメント付きの例を含めなければならない（SHALL）。

テンプレートは simple string 形式を使用し、object 形式（timeout/continue_on_failure）を使用しない。

#### Scenario: Claude template hook examples
- **WHEN** user runs `cflx init --template claude`
- **THEN** hooks セクションは各 hook 種別のコメント例を含む
- **AND** 各例は利用可能なプレースホルダーを `echo` で示す
- **AND** 例は object 形式を使用しない

#### Scenario: on_start hook example shows available placeholders
- **GIVEN** the generated template
- **THEN** on_start example is `echo '[on_start] changes_processed={changes_processed} total={total_changes} remaining={remaining_changes}'`

#### Scenario: on_change_start hook example shows available placeholders
- **GIVEN** the generated template
- **THEN** on_change_start example is `echo '[on_change_start] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'`

#### Scenario: pre_apply hook example shows available placeholders
- **GIVEN** the generated template
- **THEN** pre_apply example is `echo '[pre_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'`

#### Scenario: on_merged hook example shows available placeholders
- **GIVEN** the generated template
- **THEN** on_merged example is `echo '[on_merged] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'`

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

### Requirement: on_merged hook
オーケストレーターはchangeがbase branchにマージされた直後に`on_merged`フックを実行しなければならない（SHALL）。

`on_merged`はマージ成功時のみ1回実行され、マージ失敗時には実行しない。

parallelモードでは、自動マージが成功した全ての経路で`on_merged`を実行しなければならない（SHALL）。

#### Scenario: Parallelモードで自動マージ完了
- **GIVEN** `hooks.on_merged`が`echo 'Merged {change_id}'`に設定されている
- **WHEN** parallelモードでchange`change-a`がbase branchにマージされ`MergeCompleted`が発行される
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: Parallelモードでarchive直後に即時マージ成功
- **GIVEN** `hooks.on_merged`が`echo 'Merged {change_id}'`に設定されている
- **AND** parallelモードでchange`change-a`がarchive完了後に即時マージされる
- **WHEN** マージが成功する
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: TUI Worktreeの手動マージ完了
- **GIVEN** `hooks.on_merged`が設定されている
- **AND** worktreeブランチ`change-a`をMキーでマージする
- **WHEN** `BranchMergeCompleted`が発行される
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: serial(run)でのマージ相当
- **GIVEN** runモード（非parallel）でchange`change-a`を処理している
- **WHEN** archiveが成功し、base branchに変更が反映済みと確認できる
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

### Requirement: CLI Hook Output Visibility

The orchestrator SHALL surface hook command execution and captured hook output in normal CLI (`cflx run`) user-visible logs for every configured hook type.

#### Scenario: CLI run shows stdout from change hook

- **GIVEN** `hooks.pre_apply` is set to `echo 'hello from hook'`
- **AND** `cflx run` processes a change that executes `pre_apply`
- **WHEN** the hook completes
- **THEN** the CLI log shows the executed hook command
- **AND** the CLI log shows `hello from hook`

#### Scenario: CLI run shows stderr from change hook

- **GIVEN** `hooks.pre_apply` is set to `sh -c "echo 'hook warning' 1>&2"`
- **AND** `cflx run` processes a change that executes `pre_apply`
- **WHEN** the hook completes
- **THEN** the CLI log shows the executed hook command
- **AND** the CLI log shows the captured stderr output

#### Scenario: CLI run shows output from global hook without change id

- **GIVEN** `hooks.on_start` is set to `echo 'starting run'`
- **WHEN** `cflx run` starts orchestration
- **THEN** the CLI log shows the executed `on_start` hook command
- **AND** the CLI log shows `starting run`

#### Scenario: Hook failure still emits captured output

- **GIVEN** `hooks.post_apply` writes output and then exits non-zero
- **AND** `continue_on_failure` is `false`
- **WHEN** the hook fails during `cflx run`
- **THEN** any captured hook output is shown in the CLI log before the failure is reported
- **AND** the failure result still terminates or propagates according to hook configuration

#### Scenario: Truncated CLI hook output is marked explicitly

- **GIVEN** a configured hook writes output longer than the CLI display limit
- **WHEN** `cflx run` logs the captured hook output
- **THEN** the CLI log includes the visible prefix of the output
- **AND** the CLI log explicitly indicates that the output was truncated
