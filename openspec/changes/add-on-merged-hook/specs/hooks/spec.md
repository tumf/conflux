## ADDED Requirements
### Requirement: on_merged hook
オーケストレーターは change が base branch にマージされた直後に `on_merged` フックを実行しなければならない（SHALL）。

`on_merged` はマージ成功時のみ 1 回実行され、マージ失敗時には実行しない。

#### Scenario: Parallel モードで自動マージ完了
- **GIVEN** `hooks.on_merged` が `echo 'Merged {change_id}'` に設定されている
- **WHEN** parallel モードで change `change-a` が base branch にマージされ `MergeCompleted` が発行される
- **THEN** `on_merged` が `{change_id}=change-a` で実行される

#### Scenario: TUI Worktree の手動マージ完了
- **GIVEN** `hooks.on_merged` が設定されている
- **AND** worktree ブランチ `change-a` を M キーでマージする
- **WHEN** `BranchMergeCompleted` が発行される
- **THEN** `on_merged` が `{change_id}=change-a` で実行される

#### Scenario: serial(run) でのマージ相当
- **GIVEN** run モード（非 parallel）で change `change-a` を処理している
- **WHEN** archive が成功し、base branch に変更が反映済みと確認できる
- **THEN** `on_merged` が `{change_id}=change-a` で実行される

## MODIFIED Requirements
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
