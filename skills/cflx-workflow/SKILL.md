---
name: cflx-workflow
description: Comprehensive guide for Conflux workflow orchestration operations including TUI interaction, change processing (apply, acceptance, archive, resolve, merge), approval management, and parallel execution with worktrees. Use when working with OpenSpec change workflows, processing changes through the orchestrator, managing change states, or troubleshooting orchestration issues.
---

# Conflux Workflow Operations

Guide for orchestrating OpenSpec change workflows with Conflux.

## Overview

Conflux automates the OpenSpec change workflow through an interactive TUI or headless mode:

```
list → analyze → apply → accept → archive → resolve → merge
```

## Quick Start

### Interactive TUI (Primary Interface)

Launch the interactive dashboard:

```bash
cflx
```

**Key bindings:**

| Key | Action | Context |
|-----|--------|---------|
| `↑/↓` or `j/k` | Navigate changes | All modes |
| `Tab` | Switch between Changes/Worktrees views | All modes |
| `Space` | Toggle execution mark (Select mode) / Dynamic queue (Running mode) | All modes |
| `@` | Toggle approval | All modes |
| `F5` | Start processing marked changes | Select mode |
| `e` | Open change in editor | All modes |
| `w` | Show QR code for web monitoring | All modes |
| `M` | Trigger merge resolution | When status is `merge wait` |
| `q` | Quit | All modes |

### Headless Mode

Process all pending changes:

```bash
cflx run
```

Process specific changes:

```bash
# Single change
cflx run --change add-feature-x

# Multiple changes (comma-separated)
cflx run --change add-feature-x,fix-bug-y
```

## Change States

### Approval State

| Symbol | State | Description |
|--------|-------|-------------|
| `[ ]` | Unapproved | Cannot be selected for processing |
| `[@]` | Approved (not selected) | Ready to be selected |
| `[x]` | Selected (reserved) | Will be queued when F5 is pressed |

**Approve/unapprove changes:**

```bash
# Approve a change (creates checksums)
cflx approve set add-feature-x

# Check approval status
cflx approve status add-feature-x

# Unapprove a change
cflx approve unset add-feature-x
```

### Processing State

| Status | Description |
|--------|-------------|
| `[not queued]` | Not in execution queue |
| `[queued]` | Waiting to be processed |
| `[blocked]` | Blocked by unresolved dependencies |
| `[merge wait]` | Waiting for merge resolution (use `M`) |
| `[resolve pending]` | Resolve requested, waiting for execution |
| `[applying]` | Applying changes (shows progress %) |
| `[accepting]` | Running acceptance tests |
| `[archiving]` | Archiving to specs/ |
| `[resolving]` | Resolving merge conflicts |
| `[archived]` | Successfully archived |
| `[merged]` | Merged to base branch (parallel mode) |
| `[error]` | Processing failed |

### Header Status

| Display | Meaning |
|---------|---------|
| `[Ready]` | Selection/idle mode (`AppMode::Select`) |
| `[Running N]` | Active processing (N = active tasks) |

## Workflow Steps

### 1. Apply

Execute tasks from `tasks.md` to implement the change.

**Command:** Configured via `apply_command` in `.cflx.jsonc`

**Iteration:** Creates WIP commits for each task iteration

**Resume:** Automatically resumes from last WIP commit

### 2. Acceptance (Optional)

Run validation tests after apply completes.

**Command:** Configured via `acceptance_command` in `.cflx.jsonc`

**Result:**
- `PASS`: Proceeds to archive
- `CONTINUE`: Retries (up to `acceptance_max_continues` times)
- `FAIL`: Stops with error

**Skip acceptance:** Set `acceptance_command` to empty string in config

### 3. Archive

Archive completed changes to `openspec/specs/`.

**Command:** Configured via `archive_command` in `.cflx.jsonc`

**Actions:**
- Copy spec deltas to `openspec/specs/`
- Create archive commit
- Move change directory to `openspec/archived/`

### 4. Resolve (Parallel Mode)

Resolve merge conflicts when parallel changes conflict.

**Trigger:** Status shows `[merge wait]`, press `M` key

**Command:** Configured via `resolve_command` in `.cflx.jsonc`

### 5. Merge (Parallel Mode)

Merge worktree branch back to base branch.

**Automatic:** After successful archive (conflict-free)

**Manual:** Press `M` key after resolve completes

## Parallel Execution

### Worktree Management

**Create worktree for proposal:**

Press `+` in Worktrees view (TUI) or use configured `worktree_command`

**Delete worktree:**

Navigate to worktree and press `D` in Worktrees view

**View worktree details:**

Tab to Worktrees view to see:
- Branch name
- Workspace path
- Current state
- Conflict detection status

### Dynamic Queue

Add/remove changes during execution:

1. Navigate to change in Changes view
2. Press `Space` to toggle between `[not queued]` ⇄ `[queued]`
3. Queued changes start immediately when slots available

**Re-analysis:** 10-second debounce when queue changes

### Workspace State Detection

Enable idempotent resume:

| State | Detection | Action |
|-------|-----------|--------|
| Created | No commits | Start apply |
| Applying | WIP commits exist | Resume from iteration |
| Applied | Apply commit exists | Run archive only |
| Archived | Archive commit exists | Run merge only |
| Merged | In base branch | Cleanup workspace |

## Dependency Analysis

Use AI agent to analyze dependencies and select next change:

```
Selection criteria:
1. No dependencies, or dependencies completed
2. Higher progress (continuity)
3. Infer dependencies from change names
```

**Command:** Configured via `analyze_command` in `.cflx.jsonc`

## Retry and History

### Automatic Retry

Commands retry on transient errors:
- Module resolution failures
- Network issues
- Temporary file locks

**Configuration:**
- `command_queue_max_retries` (default: 3)
- `command_queue_stagger_delay_ms` (default: 2000ms)

### Retry Context History

Track apply/archive/resolve attempts in memory, injected into prompts for learning from failures.

**Module:** `src/history.rs`

## Lifecycle Hooks

Execute custom commands at workflow stages. See [references/hooks.md](references/hooks.md) for detailed hook configuration.

## Web Monitoring

Optional HTTP server with REST API and WebSocket:

```bash
# Enable with environment variable
WEB_ENABLED=true cflx
```

**Default URL:** http://localhost:3030

**Show QR code:** Press `w` in TUI

## Troubleshooting

### Common Issues

**Change stuck in `[applying]`:**
- Check agent logs for errors
- Verify `apply_command` configuration
- Check for stalled WIP commits (3+ empty commits)

**Approval checksum mismatch:**
- Change was modified after approval
- Re-approve with `cflx approve set <change_id>`

**Merge conflicts in parallel mode:**
- Status shows `[merge wait]`
- Press `M` to trigger resolve
- AI agent resolves conflicts via `resolve_command`

**Dynamic queue not updating:**
- Wait for 10-second debounce period
- Check dependency analysis completion

### Logging

Configure log levels with `RUST_LOG`:

```bash
# Debug level
RUST_LOG=debug cflx

# Module-specific logging
RUST_LOG=cflx=debug,cflx::orchestrator=trace cflx
```

**Log configuration:**

```jsonc
{
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  }
}
```

### State Files

- `.cflx/state.json`: Orchestrator state
- `.cflx/approval/`: Approval checksums
- `.cflx/worktrees/`: Parallel worktree metadata

## References

- [Lifecycle Hooks Configuration](references/hooks.md) - Detailed hook examples and placeholders
- [Command Templates](references/commands.md) - Agent command configuration examples
- [Parallel Execution Details](references/parallel.md) - Advanced parallel execution concepts

## Related

For configuration file management, see the `cflx-config` skill.
