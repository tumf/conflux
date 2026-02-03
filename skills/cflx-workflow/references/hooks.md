# Lifecycle Hooks Configuration

Execute custom commands at various stages of the orchestration process.

## Configuration Format

Hooks are defined in the `hooks` section of `.cflx.jsonc`:

```jsonc
{
  "hooks": {
    // Simple string format (uses default settings)
    "on_start": "echo 'Orchestrator started'",

    // Object format (with detailed settings)
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": false,  // Stop orchestration if command fails
      "timeout": 300                 // Timeout in seconds
    }
  }
}
```

## Available Hooks

### Run Lifecycle Hooks

| Hook Name | Trigger | Description |
|-----------|---------|-------------|
| `on_start` | Start | Orchestrator starts |
| `on_finish` | Finish | Orchestrator completes (success or limit) |
| `on_error` | Error | When an error occurs during apply or archive |

### Change Lifecycle Hooks

| Hook Name | Trigger | Description |
|-----------|---------|-------------|
| `on_change_start` | Change Start | When processing begins for a new change |
| `pre_apply` | Before Apply | Before applying a change |
| `post_apply` | After Apply | After successfully applying a change |
| `on_change_complete` | Task 100% | When a change reaches 100% task completion |
| `pre_archive` | Before Archive | Before archiving a change |
| `post_archive` | After Archive | After successfully archiving a change |
| `on_change_end` | Change End | After a change is successfully archived |

### TUI-Only Hooks (User Interaction)

| Hook Name | Trigger | Description |
|-----------|---------|-------------|
| `on_queue_add` | Queue Add | When user adds a change to queue (Space key) |
| `on_queue_remove` | Queue Remove | When user removes a change from queue (Space key) |
| `on_approve` | Approve | When user approves a change (@ key) |
| `on_unapprove` | Unapprove | When user unapproves a change (@ key) |

### Parallel Execution Hooks

| Hook Name | Trigger | Description |
|-----------|---------|-------------|
| `on_merged` | After Merge | After worktree branch is merged to base branch |

## Placeholders

Use placeholders in hook commands to inject runtime context:

| Placeholder | Description |
|-------------|-------------|
| `{change_id}` | Current Change ID |
| `{changes_processed}` | Number of changes processed so far |
| `{total_changes}` | Total number of changes in initial snapshot |
| `{remaining_changes}` | Remaining changes in queue |
| `{apply_count}` | Number of apply attempts for current change |
| `{completed_tasks}` | Number of completed tasks for current change |
| `{total_tasks}` | Total number of tasks for current change |
| `{status}` | Finish status (completed/iteration_limit) |
| `{error}` | Error message |

## Environment Variables

Hooks receive context via environment variables:

- `OPENSPEC_CHANGE_ID`
- `OPENSPEC_CHANGES_PROCESSED`
- `OPENSPEC_TOTAL_CHANGES`
- `OPENSPEC_REMAINING_CHANGES`
- `OPENSPEC_APPLY_COUNT`
- `OPENSPEC_COMPLETED_TASKS`
- `OPENSPEC_TOTAL_TASKS`
- `OPENSPEC_STATUS`
- `OPENSPEC_ERROR`
- `OPENSPEC_DRY_RUN`

## Examples

### Run Tests After Apply

```jsonc
{
  "hooks": {
    "post_apply": "cargo test"
  }
}
```

### Log Processing Progress

```jsonc
{
  "hooks": {
    "on_start": "echo 'Starting orchestration with {total_changes} changes'",
    "on_change_start": "echo 'Starting {change_id}'",
    "post_apply": "echo '{change_id} applied (attempt {apply_count})'",
    "on_change_end": "echo '{change_id} completed'",
    "on_finish": "echo 'Finished with status: {status}'"
  }
}
```

### Quality Gates

```jsonc
{
  "hooks": {
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": false,
      "timeout": 300
    },
    "pre_archive": {
      "command": "cargo clippy -- -D warnings",
      "continue_on_failure": false
    }
  }
}
```

### Error Logging

```jsonc
{
  "hooks": {
    "on_error": "echo 'Error in {change_id}: {error}' >> errors.log"
  }
}
```

### TUI User Actions

```jsonc
{
  "hooks": {
    "on_queue_add": "echo 'Added {change_id} to queue'",
    "on_queue_remove": "echo 'Removed {change_id} from queue'",
    "on_approve": "echo 'Approved {change_id}'",
    "on_unapprove": "echo 'Unapproved {change_id}'"
  }
}
```

### Rebuild Indexes After Merge

```jsonc
{
  "hooks": {
    "on_merged": "make bump-patch && make index"
  }
}
```

## Best Practices

1. **Use `continue_on_failure`** to control whether orchestration should stop on hook failures
2. **Set appropriate timeouts** for long-running hooks to prevent blocking
3. **Log to files** for persistent records (e.g., error logs)
4. **Use environment variables** in shell scripts for more complex hook logic
5. **Test hooks individually** before adding to orchestration workflow
