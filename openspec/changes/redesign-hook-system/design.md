# Design: Hook System Redesign

## Architecture Overview

### Current Architecture (Problem)

```
orchestrator.rs:
  loop {
    on_iteration_start  ← change_id なし
    select_next_change()
    if complete: archive
    else: apply
    on_iteration_end
  }

tui.rs:
  loop {
    Phase 1: archive_all_complete_changes()  ← on_iteration_* なし
    Phase 2: apply one change
  }
```

### New Architecture

```
Shared Orchestration Logic:
  on_start

  while pending_changes.not_empty():
    change = select_next_change()

    if is_new_change(change):
      on_change_start(change)  ← 新規

    if change.is_complete():
      on_change_complete(change)
      pre_archive(change)
      archive(change)
      post_archive(change)
      on_change_end(change)  ← 新規
    else:
      pre_apply(change)
      apply(change)
      post_apply(change)

      if change.is_now_complete():
        # 次のループで archive される
        pass

  on_finish
```

## State Management

### Change Tracking

チェンジセットの切り替えを検知するために、処理中のチェンジセット ID を追跡する：

```rust
struct Orchestrator {
    // 現在処理中のチェンジセット ID
    current_change_id: Option<String>,

    // 処理済みチェンジセット（on_change_end 呼び出し済み）
    completed_change_ids: HashSet<String>,
}
```

### Hook Trigger Logic

```rust
fn process_change(&mut self, change: &Change) {
    let is_new = self.current_change_id.as_ref() != Some(&change.id);

    if is_new {
        // 前のチェンジセットの終了処理
        if let Some(prev_id) = &self.current_change_id {
            if !self.completed_change_ids.contains(prev_id) {
                self.run_hook(OnChangeEnd, prev_context);
            }
        }

        // 新しいチェンジセットの開始処理
        self.current_change_id = Some(change.id.clone());
        self.run_hook(OnChangeStart, context);
    }

    // apply または archive の処理...
}
```

## Hook Types (New)

```rust
pub enum HookType {
    // Run lifecycle
    /// Run loop start (once)
    OnStart,
    /// Run loop finished (once)
    OnFinish,
    /// On error
    OnError,

    // Change lifecycle
    /// Change processing started (once per change)
    OnChangeStart,
    /// Before each apply execution
    PreApply,
    /// After successful apply
    PostApply,
    /// When change reaches 100% task completion
    OnChangeComplete,
    /// Before archive execution
    PreArchive,
    /// After successful archive
    PostArchive,
    /// Change processing ended (once per change, after archive)
    OnChangeEnd,

    // User interaction (TUI only)
    /// User dynamically added a change to queue (Space key)
    OnQueueAdd,
    /// User dynamically removed a change from queue (Space key)
    OnQueueRemove,
    /// User approved a change (@ key)
    OnApprove,
    /// User removed approval from a change (@ key)
    OnUnapprove,
}
```

## HookContext Updates

```rust
pub struct HookContext {
    /// Current change ID (always set except for on_start/on_finish)
    pub change_id: Option<String>,

    /// Number of changes processed so far (completed + archived)
    pub changes_processed: usize,

    /// Total number of changes in initial queue
    pub total_changes: usize,

    /// Remaining changes in queue
    pub remaining_changes: usize,

    /// Completed tasks for current change
    pub completed_tasks: Option<u32>,

    /// Total tasks for current change
    pub total_tasks: Option<u32>,

    /// Apply count for current change (how many times applied)
    pub apply_count: u32,

    /// Finish status (for on_finish: "completed", "iteration_limit", "cancelled")
    pub status: Option<String>,

    /// Error message (for on_error)
    pub error: Option<String>,
}
```

## Environment Variables

| Variable | Description | Available in |
|----------|-------------|--------------|
| OPENSPEC_CHANGE_ID | Current change ID | All except on_start/on_finish |
| OPENSPEC_CHANGES_PROCESSED | Completed changes count | All |
| OPENSPEC_TOTAL_CHANGES | Initial queue size | All |
| OPENSPEC_REMAINING_CHANGES | Remaining queue size | All |
| OPENSPEC_COMPLETED_TASKS | Tasks done in change | Change-specific hooks |
| OPENSPEC_TOTAL_TASKS | Total tasks in change | Change-specific hooks |
| OPENSPEC_APPLY_COUNT | Times this change was applied | Change-specific hooks |
| OPENSPEC_STATUS | Finish status | on_finish |
| OPENSPEC_ERROR | Error message | on_error |

## Placeholder Expansion

| Placeholder | Description |
|-------------|-------------|
| {change_id} | Current change ID |
| {changes_processed} | Completed changes count |
| {total_changes} | Initial queue size |
| {remaining_changes} | Remaining queue size |
| {completed_tasks} | Tasks done in change |
| {total_tasks} | Total tasks in change |
| {apply_count} | Times this change was applied |
| {status} | Finish status |
| {error} | Error message |

## TUI/CLI Unification

Both modes will use the same hook execution logic:

```rust
// Shared trait or function
async fn execute_change_lifecycle(
    change: &Change,
    hooks: &HookRunner,
    state: &mut OrchestrationState,
) -> Result<()> {
    // ... unified hook calling logic
}
```

## Migration from Old Hooks

| Old Hook | New Equivalent | Migration Notes |
|----------|---------------|-----------------|
| on_first_apply | on_change_start | Check `changes_processed == 0` if needed |
| on_iteration_start | on_change_start | Now has change_id |
| on_iteration_end | on_change_end | Only called after archive |
| on_queue_change | (removed) | Use on_change_end to track progress |
| pre_apply | pre_apply | Unchanged |
| post_apply | post_apply | Unchanged |
| on_change_complete | on_change_complete | Unchanged |
| pre_archive | pre_archive | Unchanged |
| post_archive | post_archive | Unchanged |
| on_start | on_start | Unchanged |
| on_finish | on_finish | Unchanged |
| on_error | on_error | Unchanged |

## Configuration Template Example

```jsonc
{
  // ... other config ...

  // Lifecycle hooks (optional)
  "hooks": {
    // === Run lifecycle ===
    // "on_start": "echo '[on_start] changes_processed={changes_processed} total={total_changes} remaining={remaining_changes}'",
    // "on_finish": "echo '[on_finish] status={status} processed={changes_processed}/{total_changes}'",
    // "on_error": "echo '[on_error] change={change_id} error={error}'",

    // === Change lifecycle ===
    // "on_change_start": "echo '[on_change_start] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'",
    // "pre_apply": "echo '[pre_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'",
    // "post_apply": "echo '[post_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'",
    // "on_change_complete": "echo '[on_change_complete] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "pre_archive": "echo '[pre_archive] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "post_archive": "echo '[post_archive] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "on_change_end": "echo '[on_change_end] change={change_id} processed={changes_processed}/{total_changes}'",

    // === User interaction (TUI only) ===
    // "on_queue_add": "echo '[on_queue_add] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "on_queue_remove": "echo '[on_queue_remove] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "on_approve": "echo '[on_approve] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "on_unapprove": "echo '[on_unapprove] change={change_id} tasks={completed_tasks}/{total_tasks}'"
  }
}
```

Note: Advanced options (`timeout`, `continue_on_failure`) are documented but not shown in templates.

## Edge Cases

### Change with 0 tasks

- on_change_start → on_change_complete → pre_archive → post_archive → on_change_end
- No apply hooks called

### Apply fails

- on_change_start → pre_apply → on_error
- on_change_end NOT called (change not completed)

### Graceful stop mid-change

- Current apply completes
- post_apply called
- on_finish called with status="cancelled"
- on_change_end NOT called for incomplete change

### Force stop

- on_error called (if possible)
- on_finish NOT guaranteed
