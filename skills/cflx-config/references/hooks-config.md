# Hook Configuration Reference

Comprehensive examples for configuring lifecycle hooks.

## Basic Syntax

### String Format

Simplest format with default settings:

```jsonc
{
  "hooks": {
    "on_start": "echo 'Starting orchestration'"
  }
}
```

### Object Format

Detailed configuration with options:

```jsonc
{
  "hooks": {
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": false,
      "timeout": 300
    }
  }
}
```

**Options:**
- `command`: Shell command to execute (required)
- `continue_on_failure`: Continue orchestration if hook fails (default: true)
- `timeout`: Maximum execution time in seconds (default: no timeout)

## Hook Categories

### Run Lifecycle Hooks

Execute at orchestrator start/finish:

```jsonc
{
  "hooks": {
    "on_start": "echo 'Starting with {total_changes} changes'",
    "on_finish": "echo 'Finished with status: {status}'",
    "on_error": "echo 'Error: {error}' >> errors.log"
  }
}
```

### Change Lifecycle Hooks

Execute during change processing:

```jsonc
{
  "hooks": {
    "on_change_start": "echo 'Starting {change_id}'",
    "pre_apply": "echo 'Applying {change_id} (attempt {apply_count})'",
    "post_apply": "cargo test",
    "on_change_complete": "echo '{change_id} reached 100% completion'",
    "pre_archive": "cargo clippy -- -D warnings",
    "post_archive": "git push origin main",
    "on_change_end": "echo 'Completed {change_id}'"
  }
}
```

### TUI User Interaction Hooks

Execute on user actions (TUI only):

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

### Parallel Execution Hooks

Execute during parallel workflow:

```jsonc
{
  "hooks": {
    "on_merged": "make bump-patch && make index"
  }
}
```

## Common Use Cases

### Quality Gates

Run tests and linting at critical points:

```jsonc
{
  "hooks": {
    "post_apply": {
      "command": "cargo test --all-features",
      "continue_on_failure": false,
      "timeout": 600
    },
    "pre_archive": {
      "command": "cargo clippy -- -D warnings && cargo fmt --check",
      "continue_on_failure": false,
      "timeout": 300
    }
  }
}
```

### Notifications

Send notifications for important events:

```jsonc
{
  "hooks": {
    "on_start": "slack-notify 'Orchestration started'",
    "on_change_complete": "slack-notify '{change_id} completed'",
    "on_error": "slack-notify 'Error in {change_id}: {error}'",
    "on_finish": "slack-notify 'Orchestration finished: {status}'"
  }
}
```

### Logging

Maintain detailed logs:

```jsonc
{
  "hooks": {
    "on_start": "echo '[START] {total_changes} changes' >> orchestration.log",
    "pre_apply": "echo '[APPLY] {change_id} attempt {apply_count}' >> orchestration.log",
    "post_apply": "echo '[APPLIED] {change_id}' >> orchestration.log",
    "pre_archive": "echo '[ARCHIVE] {change_id}' >> orchestration.log",
    "on_error": "echo '[ERROR] {change_id}: {error}' >> orchestration.log",
    "on_finish": "echo '[FINISH] {status}' >> orchestration.log"
  }
}
```

### Metrics Collection

Track performance metrics:

```jsonc
{
  "hooks": {
    "pre_apply": "metrics-start {change_id}",
    "post_apply": "metrics-end {change_id} apply",
    "post_archive": "metrics-end {change_id} archive",
    "on_finish": "metrics-summary {changes_processed} {status}"
  }
}
```

### Deployment Automation

Trigger deployments after archiving:

```jsonc
{
  "hooks": {
    "post_archive": {
      "command": "git push origin main && deploy.sh",
      "continue_on_failure": true,
      "timeout": 1800
    },
    "on_merged": "kubectl apply -f manifests/"
  }
}
```

### Cleanup Tasks

Clean up artifacts and temporary files:

```jsonc
{
  "hooks": {
    "on_change_end": "rm -rf tmp/{change_id}",
    "on_finish": "git gc && git worktree prune"
  }
}
```

### Documentation Generation

Generate documentation after changes:

```jsonc
{
  "hooks": {
    "post_archive": "cargo doc --no-deps",
    "on_merged": "mkdocs build && mkdocs gh-deploy"
  }
}
```

## Advanced Examples

### Conditional Execution

Use shell conditionals for complex logic:

```jsonc
{
  "hooks": {
    "post_apply": "if [ $OPENSPEC_APPLY_COUNT -gt 3 ]; then echo 'Warning: Multiple retries'; fi"
  }
}
```

### Parallel Hook Execution

Run multiple commands in background:

```jsonc
{
  "hooks": {
    "on_change_complete": "notify.sh {change_id} & metrics.sh {change_id} & wait"
  }
}
```

### Script Invocation

Execute external scripts:

```jsonc
{
  "hooks": {
    "pre_apply": "./scripts/pre-apply.sh {change_id}",
    "post_apply": "./scripts/post-apply.sh {change_id} {completed_tasks} {total_tasks}"
  }
}
```

**Example script (`scripts/pre-apply.sh`):**

```bash
#!/bin/bash
CHANGE_ID=$1

echo "Starting apply for $CHANGE_ID"
echo "Environment: $OPENSPEC_CHANGE_ID"
echo "Attempt: $OPENSPEC_APPLY_COUNT"
```

### Multi-Step Hooks

Chain multiple commands:

```jsonc
{
  "hooks": {
    "post_archive": "cargo build --release && cargo test && git push"
  }
}
```

### Timeout Handling

Set appropriate timeouts for long operations:

```jsonc
{
  "hooks": {
    "post_apply": {
      "command": "cargo test --all-features",
      "timeout": 600,
      "continue_on_failure": false
    },
    "post_archive": {
      "command": "deploy.sh",
      "timeout": 1800,
      "continue_on_failure": true
    }
  }
}
```

## Environment Variables

Hooks receive context via environment variables:

### Available Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `OPENSPEC_CHANGE_ID` | Current change ID | `add-feature-x` |
| `OPENSPEC_CHANGES_PROCESSED` | Changes completed | `5` |
| `OPENSPEC_TOTAL_CHANGES` | Total changes | `10` |
| `OPENSPEC_REMAINING_CHANGES` | Remaining changes | `5` |
| `OPENSPEC_APPLY_COUNT` | Apply attempts | `2` |
| `OPENSPEC_COMPLETED_TASKS` | Tasks completed | `3` |
| `OPENSPEC_TOTAL_TASKS` | Total tasks | `5` |
| `OPENSPEC_STATUS` | Finish status | `completed` |
| `OPENSPEC_ERROR` | Error message | `Failed to apply` |
| `OPENSPEC_DRY_RUN` | Dry run mode | `true` / `false` |

### Using Environment Variables

```jsonc
{
  "hooks": {
    "pre_apply": "echo 'Change: $OPENSPEC_CHANGE_ID, Attempt: $OPENSPEC_APPLY_COUNT'"
  }
}
```

**Shell script example:**

```bash
#!/bin/bash

if [ "$OPENSPEC_DRY_RUN" = "true" ]; then
  echo "Dry run mode, skipping deployment"
  exit 0
fi

if [ $OPENSPEC_APPLY_COUNT -gt 3 ]; then
  echo "Warning: $OPENSPEC_CHANGE_ID has been retried $OPENSPEC_APPLY_COUNT times"
  slack-notify "Change $OPENSPEC_CHANGE_ID needs attention"
fi
```

## Error Handling

### Continue on Failure

Control orchestration behavior on hook failure:

```jsonc
{
  "hooks": {
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": true  // Continue even if tests fail
    },
    "pre_archive": {
      "command": "cargo clippy -- -D warnings",
      "continue_on_failure": false  // Stop if linting fails
    }
  }
}
```

### Error Logging

Capture errors for debugging:

```jsonc
{
  "hooks": {
    "on_error": "echo '[ERROR] Change: {change_id}, Error: {error}, Time: $(date)' >> errors.log"
  }
}
```

### Retry Logic

Implement custom retry logic:

```jsonc
{
  "hooks": {
    "post_apply": "for i in 1 2 3; do cargo test && break || sleep 5; done"
  }
}
```

## Best Practices

1. **Use object format** for critical hooks to control failure behavior
2. **Set appropriate timeouts** to prevent hanging
3. **Log to files** for persistent records
4. **Use environment variables** for complex scripts
5. **Test hooks individually** before adding to orchestration
6. **Avoid blocking operations** in TUI hooks
7. **Use absolute paths** for external scripts
8. **Consider continue_on_failure** carefully for each hook
9. **Add error handling** in shell scripts
10. **Document hook behavior** in project README

## Troubleshooting

### Hook Not Executing

**Check:**
1. Hook name spelling
2. Command exists and is executable
3. Correct syntax (string or object format)
4. Logs for error messages

**Debug:**

```bash
RUST_LOG=debug cflx run
```

### Hook Fails Silently

**Solution:** Use object format with `continue_on_failure: false`:

```jsonc
{
  "hooks": {
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": false
    }
  }
}
```

### Timeout Issues

**Symptom:** Hook killed before completion

**Solution:** Increase timeout:

```jsonc
{
  "hooks": {
    "post_apply": {
      "command": "cargo test --all-features",
      "timeout": 1200
    }
  }
}
```

### Environment Variables Not Available

**Solution:** Use placeholders instead:

```jsonc
{
  "hooks": {
    "pre_apply": "echo 'Change: {change_id}, Attempt: {apply_count}'"
  }
}
```

### Shell Syntax Errors

**Symptom:** Hook fails with syntax error

**Solutions:**
1. Use proper quoting: `'single quotes'` for literal strings
2. Escape special characters: `\$`, `\\`
3. Test command manually: `bash -c "command"`
4. Use external script file for complex logic
