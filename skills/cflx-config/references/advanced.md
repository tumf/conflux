# Advanced Configuration Options

Advanced settings for fine-tuning Conflux behavior.

## Stall Detection

Detect and handle stalled apply operations with consecutive empty WIP commits.

### Configuration

```jsonc
{
  "stall_detection": {
    "enabled": true,
    "threshold": 3
  }
}
```

**Options:**
- `enabled`: Enable/disable stall detection (default: true)
- `threshold`: Number of consecutive empty WIP commits before stalling (default: 3)

### Behavior

When N consecutive empty WIP commits are detected:
1. Stop apply operation
2. Mark change as errored
3. Log stall detection event
4. Continue to next change

**Use cases:**
- Agent stuck in infinite loop
- Tasks cannot be completed due to environment issues
- Agent repeatedly applying same unsuccessful changes

### Disable Stall Detection

For long-running tasks that create many commits:

```jsonc
{
  "stall_detection": {
    "enabled": false
  }
}
```

## Command Queue Configuration

Control command execution timing and retry behavior.

### Configuration

```jsonc
{
  "command_queue_stagger_delay_ms": 2000,
  "command_queue_max_retries": 3
}
```

### Stagger Delay

**Purpose:** Prevent resource conflicts when starting multiple agents

**Default:** 2000ms (2 seconds)

**Increase for:**
- Slow startup agents
- Resource-constrained environments
- Network-dependent operations

**Decrease for:**
- Fast agents
- Powerful hardware
- Local operations only

**Example:**

```jsonc
{
  "command_queue_stagger_delay_ms": 5000  // 5 seconds between starts
}
```

### Max Retries

**Purpose:** Retry transient failures automatically

**Default:** 3 attempts

**Retryable errors:**
- Module resolution failures
- Network timeouts
- Temporary file locks
- Process spawn failures

**Non-retryable errors:**
- Syntax errors
- Permanent file not found
- Permission denied

**Example:**

```jsonc
{
  "command_queue_max_retries": 5  // Retry up to 5 times
}
```

## VCS Backend Selection

Specify version control system explicitly.

### Configuration

```jsonc
{
  "vcs_backend": "git"  // or "auto"
}
```

**Options:**
- `"auto"`: Detect VCS automatically (default)
- `"git"`: Force Git backend

**Auto detection:**
1. Check for `.git` directory
2. Fall back to Git if no VCS detected

**Explicit selection:**

Use when:
- Multiple VCS systems present
- Auto-detection unreliable
- Specific VCS features needed

## Workspace Resume Control

Control whether existing workspaces are resumed or recreated.

### Enable Resume (Default)

```jsonc
{
  // No configuration needed, resume is default
}
```

**Behavior:**
- Detect existing workspace state
- Resume from last checkpoint
- Preserve WIP commits

### Disable Resume

**Command-line:**

```bash
cflx run --parallel --no-resume
```

**Use cases:**
- Force clean workspace creation
- Reset after failed attempts
- Testing fresh state behavior

**Effect:**
- Delete existing workspaces
- Create new workspaces
- Lose WIP commit history

## Web Monitoring Configuration

Configure HTTP server for remote monitoring.

### Basic Configuration

```jsonc
{
  "web": {
    "enabled": true,
    "port": 3030,
    "bind_address": "127.0.0.1"
  }
}
```

### Command-Line Override

```bash
# Enable web server
cflx --web

# Custom port
cflx --web --web-port 8080

# Custom bind address (all interfaces)
cflx --web --web-bind 0.0.0.0
```

### Environment Variables

```bash
export WEB_ENABLED=true
export WEB_PORT=3030
export WEB_BIND=127.0.0.1
cflx
```

### Security Considerations

**Local only (default):**

```jsonc
{
  "web": {
    "bind_address": "127.0.0.1"
  }
}
```

**Remote access:**

```jsonc
{
  "web": {
    "bind_address": "0.0.0.0"
  }
}
```

**Warning:** Binding to `0.0.0.0` exposes the server to network. Use firewall rules to restrict access.

### API Endpoints

**REST API:**
- `GET /api/changes` - List all changes
- `GET /api/changes/{id}` - Get change details
- `GET /api/status` - Orchestrator status

**WebSocket:**
- `ws://localhost:3030/ws` - Real-time updates

## Workspace Base Directory

Customize workspace location for parallel execution.

### Default Locations

**macOS:**
```
~/Library/Application Support/conflux/worktrees/<project_slug>
```

**Linux:**
```
~/.local/share/conflux/worktrees/<project_slug>
```

**Windows:**
```
%APPDATA%\Conflux\worktrees\<project_slug>
```

### Custom Location

```jsonc
{
  "workspace_base_dir": "/custom/path/to/worktrees"
}
```

**Use cases:**
- Faster storage (SSD)
- Separate volume (disk space)
- Network storage (shared environments)
- Custom organization

**Example:**

```jsonc
{
  "workspace_base_dir": "/mnt/fast-ssd/conflux-workspaces"
}
```

### Project Slug Format

Workspaces are organized by project slug: `<repo_basename>-<hash8>`

**Example:** `conflux-a1b2c3d4`

**Hash:** First 8 characters of repository path hash (for uniqueness)

## Logging Configuration

Fine-tune log output and verbosity.

### Configuration

```jsonc
{
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  }
}
```

### Suppress Repetitive Debug

**Purpose:** Reduce log noise from unchanged state

**Default:** true

**Behavior:**
- Suppress repeated debug logs when state unchanged
- Continue showing info/warn/error logs
- Resume debug logs when state changes

**Disable for debugging:**

```jsonc
{
  "logging": {
    "suppress_repetitive_debug": false
  }
}
```

### Summary Interval

**Purpose:** Periodic summary logs during long operations

**Default:** 60 seconds

**Behavior:**
- Emit summary every N seconds
- Show progress and active changes
- Useful for monitoring long-running orchestrations

**Disable summaries:**

```jsonc
{
  "logging": {
    "summary_interval_secs": 0
  }
}
```

**Increase frequency:**

```jsonc
{
  "logging": {
    "summary_interval_secs": 30  // Every 30 seconds
  }
}
```

### Log Levels

**Environment variable:**

```bash
# All debug logs
RUST_LOG=debug cflx

# Module-specific
RUST_LOG=cflx=info,cflx::orchestrator=debug cflx

# Multiple modules
RUST_LOG=cflx::parallel=trace,cflx::agent=debug cflx
```

**Levels:**
- `error`: Errors only
- `warn`: Warnings and errors
- `info`: Informational messages (default)
- `debug`: Detailed debugging
- `trace`: Very verbose debugging

## Acceptance Configuration

Fine-tune acceptance testing behavior.

### Prompt Mode

```jsonc
{
  "acceptance_prompt_mode": "full"  // or "context_only"
}
```

**Options:**
- `"full"`: Include hardcoded system prompt + context (default)
- `"context_only"`: Only include context, rely on command template

**Use `"context_only"` when:**
- Using command templates (e.g., OpenCode)
- Agent has fixed acceptance instructions
- Custom acceptance workflow

**Use `"full"` when:**
- Using generic agent commands
- Need fallback acceptance instructions
- No command template available

### Max Continues

```jsonc
{
  "acceptance_max_continues": 10
}
```

**Purpose:** Limit acceptance retry attempts

**Default:** 10

**Behavior:**
- `PASS`: Proceed to archive
- `CONTINUE`: Retry (up to max_continues)
- `FAIL`: Stop with error

**Increase for flaky tests:**

```jsonc
{
  "acceptance_max_continues": 20
}
```

**Decrease for strict validation:**

```jsonc
{
  "acceptance_max_continues": 3
}
```

### Disable Acceptance

```jsonc
{
  "acceptance_command": ""
}
```

Skip acceptance testing entirely.

## Performance Tuning

Optimize for different environments.

### High Concurrency

```jsonc
{
  "max_concurrent_workspaces": 10,
  "command_queue_stagger_delay_ms": 500
}
```

**Requirements:**
- Powerful hardware (multi-core CPU, plenty RAM)
- Fast storage (SSD)
- Stable network

**Trade-offs:**
- Higher resource usage
- Faster overall completion
- More potential conflicts

### Low Resources

```jsonc
{
  "max_concurrent_workspaces": 2,
  "command_queue_stagger_delay_ms": 5000,
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 120
  }
}
```

**Suitable for:**
- Limited CPU/RAM
- Slow storage (HDD)
- Network-constrained environments

**Trade-offs:**
- Lower resource usage
- Slower overall completion
- Fewer conflicts

### CI/CD Optimization

```jsonc
{
  "max_concurrent_workspaces": 5,
  "command_queue_stagger_delay_ms": 1000,
  "acceptance_command": "",
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 30
  },
  "hooks": {
    "post_apply": {
      "command": "cargo test --all-features",
      "continue_on_failure": false,
      "timeout": 600
    }
  }
}
```

**Optimizations:**
- Moderate concurrency for CI resources
- Faster stagger delay
- Disable acceptance (use post_apply hook instead)
- Frequent summaries for monitoring
- Quality gates via hooks

## Environment-Specific Configurations

Use different configs for different environments.

### Development

**File:** `.cflx.jsonc`

```jsonc
{
  "analyze_command": "claude -p '{prompt}'",
  "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude -p '/openspec:archive {change_id}'",

  "max_concurrent_workspaces": 2,
  "logging": {
    "suppress_repetitive_debug": false,
    "summary_interval_secs": 30
  }
}
```

### Production

**File:** `.cflx.production.jsonc`

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --output-format stream-json -p '{prompt}'",
  "apply_command": "claude --dangerously-skip-permissions --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --output-format stream-json -p '/openspec:accept {change_id} {prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  "max_concurrent_workspaces": 5,
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  },
  "hooks": {
    "post_apply": "cargo test",
    "pre_archive": "cargo clippy -- -D warnings"
  }
}
```

**Usage:**

```bash
cflx run --config .cflx.production.jsonc
```

## Best Practices

1. **Start conservative** with concurrency and stagger delay
2. **Monitor resource usage** and adjust accordingly
3. **Use appropriate log levels** for environment
4. **Configure timeouts** for long-running operations
5. **Test configuration** with `--dry-run` first
6. **Document custom settings** in project README
7. **Version control configurations** for reproducibility
8. **Use environment variables** for sensitive data
9. **Benchmark different settings** to find optimal values
10. **Review logs regularly** to identify bottlenecks
