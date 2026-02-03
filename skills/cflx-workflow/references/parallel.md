# Parallel Execution Details

Advanced concepts for parallel change processing with Git worktrees.

## Overview

Parallel execution mode processes multiple independent changes simultaneously using Git worktrees. Each change gets its own isolated workspace.

## Enable Parallel Mode

```bash
# TUI (default mode)
cflx

# Headless mode
cflx run --parallel --max-concurrent 3
```

## Workspace Management

### Workspace Base Directory

**Configuration key:** `workspace_base_dir`

**Default locations (when not configured):**
- **macOS**: `~/Library/Application Support/conflux/worktrees/<project_slug>`
- **Linux**: `~/.local/share/conflux/worktrees/<project_slug>`
- **Windows**: `%APPDATA%\Conflux\worktrees\<project_slug>`

**Project slug format:** `<repo_basename>-<hash8>` (e.g., `conflux-a1b2c3d4`)

**Custom configuration:**

```jsonc
{
  "workspace_base_dir": "/path/to/custom/worktrees"
}
```

### Workspace Lifecycle

1. **Create:** Worktree created for each change
2. **Apply:** Execute tasks in isolated workspace
3. **Archive:** Copy specs to `openspec/specs/`
4. **Merge:** Merge worktree branch to base branch
5. **Cleanup:** Delete worktree after merge

### Resume Behavior

**Automatic resume (default):**

Detects workspace state and resumes from interruption:

| State | Detection | Action |
|-------|-----------|--------|
| Created | No commits | Start apply |
| Applying | WIP commits exist | Resume from iteration |
| Applied | Apply commit exists | Run archive only |
| Archived | Archive commit exists | Run merge only |
| Merged | In base branch | Cleanup workspace |

**Disable resume:**

```bash
cflx run --parallel --no-resume
```

Forces creation of new workspaces, ignoring existing state.

## Conflict Detection and Resolution

### Automatic Conflict Detection

Runs in background to detect potential merge conflicts:

- Periodic checks during execution
- Identifies conflicting file changes between worktrees
- Updates status in Worktrees view

### Conflict States

| Status | Description | Action |
|--------|-------------|--------|
| `[applying]` | No conflicts detected | Continue processing |
| `[merge wait]` | Conflict detected or manual trigger | Press `M` to resolve |
| `[resolving]` | AI agent resolving conflicts | Wait for completion |
| `[merged]` | Successfully merged | Cleanup workspace |

### Manual Resolution Trigger

Press `M` key in TUI when:
- Status shows `[merge wait]`
- Manual conflict resolution needed
- Merge needs to be retried

**Command:** Configured via `resolve_command` in `.cflx.jsonc`

### Merge Process

**Automatic merge (conflict-free):**

After successful archive, automatically merges worktree branch to base branch.

**Manual merge:**

After resolve completes, press `M` to trigger merge.

## Dynamic Queue

Add/remove changes during execution without stopping:

1. Navigate to change in Changes view
2. Press `Space` to toggle: `[not queued]` ⇄ `[queued]`
3. Queued changes start immediately when slots available

**Re-analysis trigger:**
- 10-second debounce after queue changes
- Recalculates dependencies for new selection order

## Concurrency Control

### Maximum Concurrent Workspaces

**Default:** 3

**Configure:**

```jsonc
{
  "max_concurrent_workspaces": 5
}
```

**Command-line override:**

```bash
cflx run --parallel --max-concurrent 5
```

### Command Execution Queue

Prevents resource conflicts with staggered execution:

**Configuration:**

```jsonc
{
  "command_queue_stagger_delay_ms": 2000,
  "command_queue_max_retries": 3
}
```

- `command_queue_stagger_delay_ms`: Delay between starting commands (default: 2000ms)
- `command_queue_max_retries`: Retry attempts for transient errors (default: 3)

**Module:** `src/command_queue.rs`

## Worktree View (TUI)

Press `Tab` to switch to Worktrees view.

**Features:**
- View all active worktrees
- See branch names and workspace paths
- Monitor current state and conflict status
- Manage worktree lifecycle

**Key bindings:**

| Key | Action |
|-----|--------|
| `+` | Create new proposal worktree |
| `D` | Delete selected worktree |
| `e` | Open worktree in editor |
| `M` | Trigger merge resolution |

## Dependency Analysis

Ensures correct execution order:

1. Analyzes change dependencies
2. Groups independent changes
3. Executes groups in parallel
4. Respects dependency ordering

**Blocked state:**

Change shows `[blocked]` when waiting for dependencies to complete.

## Process Management

Platform-specific child process cleanup:

**Unix:**
- Process groups with `setpgid()`
- Cleanup with `killpg()`

**Windows:**
- Job objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
- Automatic cleanup on parent exit

**Module:** `src/agent.rs`

## State Persistence

### Workspace State File

Each workspace maintains state in `.cflx/state.json`:

```json
{
  "change_id": "add-feature-x",
  "workspace_path": "/path/to/worktree",
  "branch_name": "cflx/add-feature-x",
  "state": "applying",
  "wip_commits": 3,
  "last_updated": "2024-01-01T12:00:00Z"
}
```

### Global Orchestrator State

Maintains overall orchestration state in `.cflx/state.json`:

```json
{
  "mode": "parallel",
  "active_workspaces": ["add-feature-x", "fix-bug-y"],
  "queued_changes": ["refactor-z"],
  "completed_changes": []
}
```

## Troubleshooting

### Workspace Stuck in Applying

**Symptoms:**
- Change shows `[applying]` indefinitely
- No progress updates

**Solutions:**
1. Check agent logs for errors
2. Verify workspace still exists: `ls -la <workspace_path>`
3. Check for stalled WIP commits (3+ empty commits)
4. Manually inspect workspace: `cd <workspace_path>`
5. Delete and recreate: `cflx run --parallel --no-resume`

### Merge Conflicts Not Resolving

**Symptoms:**
- Status stuck in `[merge wait]` or `[resolving]`
- Merge attempts fail repeatedly

**Solutions:**
1. Check `resolve_command` configuration
2. Manually inspect conflicts: `cd <workspace_path> && git status`
3. Review resolve agent logs
4. Manually resolve and commit, then press `M`

### Worktree Cleanup Issues

**Symptoms:**
- Old worktrees not deleted
- Disk space issues

**Solutions:**
1. List worktrees: `git worktree list`
2. Manual cleanup: `git worktree remove <path>`
3. Force cleanup: `git worktree remove --force <path>`
4. Prune invalid entries: `git worktree prune`

### Dynamic Queue Not Updating

**Symptoms:**
- Queue changes not taking effect
- New changes not starting

**Solutions:**
1. Wait for 10-second debounce period
2. Check dependency analysis completion
3. Verify max_concurrent_workspaces not reached
4. Check for blocked dependencies

## Performance Tuning

### Optimize Concurrency

```jsonc
{
  "max_concurrent_workspaces": 5,  // Increase for more parallelism
  "command_queue_stagger_delay_ms": 1000  // Reduce for faster startup
}
```

**Trade-offs:**
- Higher concurrency = more resource usage
- Lower stagger delay = higher conflict risk

### Disk Space Management

Monitor workspace base directory size:

```bash
du -sh ~/Library/Application\ Support/conflux/worktrees/
```

Clean up old worktrees:

```bash
git worktree prune
rm -rf ~/Library/Application\ Support/conflux/worktrees/<project_slug>/*
```

### Network Optimization

For remote repositories, use local mirrors to reduce fetch times:

```bash
git clone --mirror https://github.com/org/repo.git /path/to/mirror
git -C /path/to/mirror remote update
```

Configure worktrees to use mirror:

```jsonc
{
  "vcs_backend": "git",
  "git_mirror_path": "/path/to/mirror"
}
```

## Best Practices

1. **Start with low concurrency** (2-3) and increase as needed
2. **Monitor disk space** in workspace base directory
3. **Use --dry-run** to preview parallelization before executing
4. **Enable web monitoring** for remote visibility (`--web`)
5. **Configure appropriate stagger delay** for your environment
6. **Review conflict resolution** commands before enabling parallel mode
7. **Test resume behavior** by intentionally interrupting execution
8. **Use lifecycle hooks** to clean up resources after merge
