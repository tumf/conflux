# hooks Specification

## Purpose
Defines the lifecycle hook system including available hooks, context variables, and execution order.
## Requirements

### Requirement: on_queue_add hook

The orchestrator SHALL execute `on_queue_add` exactly once after the shared operator command service successfully adds a change to the dynamic queue, regardless of whether the request originated from TUI or another frontend. Initial queue construction, rejected requests, and no-op duplicate additions SHALL NOT execute the hook.

#### Scenario: TUI adds change to queue

- **GIVEN** `hooks.on_queue_add` is configured
- **AND** TUI is in Running or Stopped mode
- **WHEN** the user presses Space on an eligible unqueued change
- **THEN** the shared operator command service mutates the dynamic queue
- **AND** `on_queue_add` executes exactly once with the change ID

#### Scenario: Remote frontend adds change to queue

- **GIVEN** `hooks.on_queue_add` is configured
- **WHEN** a remote frontend requests an eligible dynamic queue addition through the shared operator command service
- **THEN** the queue mutation and hook behavior are identical to the TUI path

#### Scenario: on_queue_add not called for initial queue

- **GIVEN** `hooks.on_queue_add` is configured
- **AND** the operator marks changes before starting orchestration
- **WHEN** orchestration constructs its initial queue
- **THEN** `on_queue_add` is NOT called

#### Scenario: on_queue_add not called for no-op

- **GIVEN** a change is already in the dynamic queue
- **WHEN** any frontend requests the same addition
- **THEN** the request is a no-op
- **AND** `on_queue_add` is NOT called

### Requirement: on_queue_remove hook

The orchestrator SHALL execute `on_queue_remove` exactly once after the shared operator command service successfully removes a change from the dynamic queue, regardless of whether the request originated from TUI or another frontend. Rejected requests and no-op removals SHALL NOT execute the hook.

#### Scenario: TUI removes change from queue

- **GIVEN** `hooks.on_queue_remove` is configured
- **AND** TUI is in Running or Stopped mode
- **WHEN** the user presses Space on an eligible queued change
- **THEN** the shared operator command service mutates the dynamic queue
- **AND** `on_queue_remove` executes exactly once with the change ID

#### Scenario: Remote frontend removes change from queue

- **GIVEN** `hooks.on_queue_remove` is configured
- **WHEN** a remote frontend requests an eligible dynamic queue removal through the shared operator command service
- **THEN** the queue mutation and hook behavior are identical to the TUI path

#### Scenario: on_queue_remove not called for no-op

- **GIVEN** a change is not in the dynamic queue
- **WHEN** any frontend requests its removal
- **THEN** the request is a no-op
- **AND** `on_queue_remove` is NOT called

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

The orchestrator SHALL support the following hook types:

**Run lifecycle:**
- `on_start`: Run loop started
- `on_finish`: Run loop finished
- `on_error`: Error occurred

**Change lifecycle:**
- `on_change_start`: Change processing started once per change
- `pre_apply`: Before apply execution
- `post_apply`: After successful apply
- `on_change_complete`: Change reached complete task state
- `pre_archive`: Before archive execution
- `post_archive`: After successful archive
- `on_change_end`: Change processing ended after archive
- `on_merged`: Change merged to base branch

**Frontend-independent operator interaction:**
- `on_queue_add`: Shared operator service dynamically added a change to the queue
- `on_queue_remove`: Shared operator service dynamically removed a change from the queue

**TUI-only interaction:**
- `on_approve`: User approved a change with the TUI approval control
- `on_unapprove`: User removed approval with the TUI approval control

#### Scenario: Complete hook list in configuration

- **GIVEN** config contains all hook types
- **WHEN** the orchestrator loads config
- **THEN** all hooks are registered
- **AND** queue hooks are triggered by successful shared-service mutations
- **AND** approval hooks remain TUI-only

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

For repo-mutating `on_merged` commands, the hook runner SHALL provide repository-verifiable diagnostics around root `.git/index.lock` waiting and execution readiness.

At minimum, the logs SHALL make it observable whether root `.git/index.lock` was already present before hook execution, whether it was released during the configured wait window, or whether execution proceeded after timeout.

These diagnostics are observational only and MUST NOT introduce hidden out-of-worktree durable workflow-control state.

#### Scenario: pre-existing root lock is logged before hook execution

**Given**: root `.git/index.lock` exists before `on_merged` starts
**When**: the hook runner prepares `on_merged`
**Then**: the logs indicate that root lock waiting began
**And**: the logs later indicate whether the lock was released or the wait timed out

#### Scenario: timeout does not hide unsafe execution context

**Given**: root `.git/index.lock` remains present until `index_lock_wait_secs` expires
**When**: the hook runner proceeds to execute `on_merged`
**Then**: the logs explicitly indicate that execution continued after lock wait timeout
**And**: a later hook failure can be correlated with the unsafe root lock condition from repository-verifiable logs

### Requirement: CLI Hook Output Visibility

The orchestrator SHALL surface hook command execution and captured hook output in normal CLI (`cflx run`) user-visible logs for every configured hook type.

Captured hook output severity SHALL reflect hook outcome. Stderr from a hook command that exits successfully SHALL remain visible as informational hook output and SHALL NOT be classified as warning/failure solely because the stream is stderr. Stderr from a hook command that fails SHALL remain visible as warning/error context before the failure is reported.

Hook output visibility is observational only and MUST NOT introduce hidden out-of-worktree durable workflow-control state.

#### Scenario: CLI run shows stdout from change hook

- **GIVEN** `hooks.pre_apply` is set to `echo 'hello from hook'`
- **AND** `cflx run` processes a change that executes `pre_apply`
- **WHEN** the hook completes
- **THEN** the CLI log shows the executed hook command
- **AND** the CLI log shows `hello from hook`

#### Scenario: successful hook stderr remains informational

- **GIVEN** `hooks.pre_apply` is set to `sh -c "echo 'hook diagnostic' 1>&2"`
- **AND** `cflx run` processes a change that executes `pre_apply`
- **WHEN** the hook exits zero
- **THEN** the captured stderr output remains visible
- **AND** the output is not emitted as a warning-level hook failure diagnostic solely because it came from stderr

#### Scenario: Hook failure still emits captured output

- **GIVEN** `hooks.post_apply` writes stderr output and then exits non-zero
- **AND** `continue_on_failure` is `false`
- **WHEN** the hook fails during `cflx run`
- **THEN** the captured stderr output is shown in warning/error context before the failure is reported
- **AND** the failure result still terminates or propagates according to hook configuration

#### Scenario: Truncated CLI hook output is marked explicitly

- **GIVEN** a configured hook writes output longer than the CLI display limit
- **WHEN** `cflx run` logs the captured hook output
- **THEN** the CLI log includes the visible prefix of the output
- **AND** the CLI log explicitly indicates that the output was truncated

### Requirement: Hook configuration format

Hook configuration SHALL support both simple string form and detailed object form.

The detailed object form SHALL support the following fields:
- `command` (string, required): The command to execute
- `timeout` (u64, default 60): Timeout in seconds
- `continue_on_failure` (bool, default true): Whether to continue if the hook fails
- `git_commit_no_verify` (bool, default false): Whether downstream git commits should skip verification hooks
- `max_retries` (u32, default 0): Number of retries on non-zero exit before applying `continue_on_failure` logic
- `retry_delay_secs` (u64, default 3): Delay in seconds between retries

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
- **THEN** the hook is registered with default timeout (60s), continue_on_failure (true), max_retries (0), and retry_delay_secs (3)

#### Scenario: Detailed hook configuration with retry

- **GIVEN** config contains:
  ```jsonc
  {
    "hooks": {
      "on_merged": {
        "command": "make bump-patch",
        "timeout": 120,
        "git_commit_no_verify": true,
        "max_retries": 3,
        "retry_delay_secs": 3
      }
    }
  }
  ```
- **WHEN** orchestrator loads the config
- **THEN** the hook uses timeout=120s, git_commit_no_verify=true, max_retries=3, and retry_delay_secs=3

#### Scenario: Hook retry on failure

- **GIVEN** a hook is configured with `max_retries: 2` and `retry_delay_secs: 3`
- **AND** the hook command exits with non-zero status on the first attempt
- **WHEN** the hook is executed
- **THEN** the orchestrator waits 3 seconds and retries
- **AND** if the retry succeeds, the hook is considered successful
- **AND** if all retries fail, `continue_on_failure` logic is applied

#### Scenario: Default max_retries is zero (no retry)

- **GIVEN** a hook is configured without `max_retries`
- **AND** the hook command exits with non-zero status
- **WHEN** the hook is executed
- **THEN** `continue_on_failure` logic is applied immediately without retry

### Requirement: on_merged hook

For repo-mutating `on_merged` commands, the hook runner SHALL provide repository-verifiable diagnostics around root `.git/index.lock` waiting and execution readiness.

At minimum, the logs SHALL make it observable whether root `.git/index.lock` was already present before hook execution, whether it was released during the configured wait window, or whether execution proceeded after timeout.

These diagnostics are observational only and MUST NOT introduce hidden out-of-worktree durable workflow-control state.

#### Scenario: pre-existing root lock is logged before hook execution

**Given**: root `.git/index.lock` exists before `on_merged` starts
**When**: the hook runner prepares `on_merged`
**Then**: the logs indicate that root lock waiting began
**And**: the logs later indicate whether the lock was released or the wait timed out

#### Scenario: timeout does not hide unsafe execution context

**Given**: root `.git/index.lock` remains present until `index_lock_wait_secs` expires
**When**: the hook runner proceeds to execute `on_merged`
**Then**: the logs explicitly indicate that execution continued after lock wait timeout
**And**: a later hook failure can be correlated with the unsafe root lock condition from repository-verifiable logs

### Requirement: Hook execution working directory

フックコマンドは常にリポジトリルートディレクトリで実行されなければならない（SHALL）。

`HookRunner` は `repo_root` パスを保持し、`execute_hook()` でコマンドの作業ディレクトリとして設定しなければならない（SHALL）。

#### Scenario: フックがリポジトリルートで実行される

- **GIVEN** リポジトリルートが `/path/to/repo` である
- **AND** フックコマンドが `pwd` に設定されている
- **WHEN** フックが実行される
- **THEN** コマンドの出力は `/path/to/repo` である

#### Scenario: parallel mode worktree からのフック実行

- **GIVEN** parallel mode で worktree `/tmp/worktrees/change-a` が使用されている
- **AND** リポジトリルートが `/path/to/repo` である
- **WHEN** `on_merged` フックが実行される
- **THEN** コマンドはリポジトリルート `/path/to/repo` で実行される

### Requirement: on_merged hook

For repo-mutating `on_merged` commands, the hook runner SHALL provide repository-verifiable diagnostics around root `.git/index.lock` waiting and execution readiness.

At minimum, the logs SHALL make it observable whether root `.git/index.lock` was already present before hook execution, whether it was released during the configured wait window, or whether execution proceeded after timeout.

These diagnostics are observational only and MUST NOT introduce hidden out-of-worktree durable workflow-control state.

#### Scenario: pre-existing root lock is logged before hook execution

**Given**: root `.git/index.lock` exists before `on_merged` starts
**When**: the hook runner prepares `on_merged`
**Then**: the logs indicate that root lock waiting began
**And**: the logs later indicate whether the lock was released or the wait timed out

#### Scenario: timeout does not hide unsafe execution context

**Given**: root `.git/index.lock` remains present until `index_lock_wait_secs` expires
**When**: the hook runner proceeds to execute `on_merged`
**Then**: the logs explicitly indicate that execution continued after lock wait timeout
**And**: a later hook failure can be correlated with the unsafe root lock condition from repository-verifiable logs
