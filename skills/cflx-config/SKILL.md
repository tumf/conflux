---
name: cflx-config
description: Configuration management for Conflux orchestrator including JSONC configuration files, agent commands, system prompts, workspace settings, logging, VCS backend, and parallel execution options. Use when setting up Conflux, customizing agent behavior, troubleshooting configuration issues, or migrating between environments.
---

# Conflux Configuration Management

Guide for managing Conflux orchestrator configuration files.

## Overview

Conflux uses JSONC (JSON with Comments) configuration files to customize behavior, agent commands, and workflow settings.

## Configuration File Locations

Configs are merged (not replaced). Later configs override earlier ones:

| Priority | Location | Use Case |
|----------|----------|----------|
| 1 | `.cflx.jsonc` | Project-specific settings |
| 2 | `~/.config/cflx/config.jsonc` | Global user settings |
| 3 | `--config <path>` | Custom path override |

## Quick Start

### Initialize Configuration

Generate a configuration file:

```bash
cflx init
```

Or copy from example:

```bash
cp .cflx.jsonc.example .cflx.jsonc
vim .cflx.jsonc
```

### Basic Configuration

Minimal configuration for Claude Code:

```jsonc
{
  "analyze_command": "claude -p '{prompt}'",
  "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude -p '/openspec:archive {change_id} {prompt}'"
}
```

## Configuration Schema

### Agent Commands

Configure AI agent commands for workflow operations.

**Required commands:**

```jsonc
{
  "analyze_command": "claude -p '{prompt}'",
  "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude -p '/openspec:archive {change_id} {prompt}'"
}
```

**Optional commands:**

```jsonc
{
  "acceptance_command": "claude -p '/openspec:accept {change_id} {prompt}'",
  "resolve_command": "claude -p '{prompt}'",
  "worktree_command": "claude -p '/openspec:proposal --worktree {workspace_dir}'"
}
```

See [references/agent-configs.md](references/agent-configs.md) for detailed agent configuration examples.

### System Prompts

Customize prompts injected into agent commands:

```jsonc
{
  "apply_prompt": "Custom instructions for apply operation",
  "acceptance_prompt": "Custom instructions for acceptance testing",
  "archive_prompt": "Custom instructions for archiving"
}
```

**Acceptance prompt modes:**

```jsonc
{
  "acceptance_prompt_mode": "full",  // or "context_only"
  "acceptance_max_continues": 10
}
```

- `"full"`: Include hardcoded system prompt + context (default)
- `"context_only"`: Only include context, use command template for instructions

### Parallel Execution

Configure parallel processing with worktrees:

```jsonc
{
  "max_concurrent_workspaces": 3,
  "workspace_base_dir": "/custom/path/to/worktrees"
}
```

**Default workspace locations:**
- macOS: `~/Library/Application Support/conflux/worktrees/<project_slug>`
- Linux: `~/.local/share/conflux/worktrees/<project_slug>`
- Windows: `%APPDATA%\Conflux\worktrees\<project_slug>`

### Command Queue

Control command execution and retry behavior:

```jsonc
{
  "command_queue_stagger_delay_ms": 2000,
  "command_queue_max_retries": 3
}
```

### Logging

Configure log output and verbosity:

```jsonc
{
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  }
}
```

### Stall Detection

Detect and handle stalled apply operations:

```jsonc
{
  "stall_detection": {
    "enabled": true,
    "threshold": 3
  }
}
```

Stops after N consecutive empty WIP commits (default: 3).

### VCS Backend

Specify version control system:

```jsonc
{
  "vcs_backend": "git"  // or "auto" (default)
}
```

### Lifecycle Hooks

Execute custom commands at workflow stages:

```jsonc
{
  "hooks": {
    "on_start": "echo 'Starting orchestration'",
    "post_apply": "cargo test",
    "on_merged": "make bump-patch && make index"
  }
}
```

See [references/hooks-config.md](references/hooks-config.md) for detailed hook configuration.

### Web Monitoring

Configure web monitoring server:

```jsonc
{
  "web": {
    "enabled": false,
    "port": 3030,
    "bind_address": "127.0.0.1"
  }
}
```

## Configuration Templates

### Claude Code

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:accept {change_id} {prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "worktree_command": "claude --dangerously-skip-permissions --verbose -p '/openspec:proposal --worktree {workspace_dir}'"
}
```

### OpenCode

```jsonc
{
  "analyze_command": "opencode run --format json {prompt}",
  "apply_command": "opencode run '/openspec-apply {change_id}'",
  "acceptance_command": "opencode run '/cflx-accept {change_id} {prompt}'",
  "archive_command": "opencode run '/conflux:archive {change_id}'",
  "resolve_command": "opencode run {prompt}"
}
```

### Codex

```jsonc
{
  "analyze_command": "codex run '{prompt}'",
  "apply_command": "codex run '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "codex run '/openspec:accept {change_id} {prompt}'",
  "archive_command": "codex run '/openspec:archive {change_id} {prompt}'",
  "resolve_command": "codex run '{prompt}'"
}
```

## Environment Variables

Override configuration with environment variables:

```bash
# OpenSpec command
export OPENSPEC_CMD="npx @fission-ai/openspec@latest"

# Logging level
export RUST_LOG=debug

# Web monitoring
export WEB_ENABLED=true
export WEB_PORT=3030
```

## Placeholders

Use in command and hook configurations:

### Command Placeholders

| Placeholder | Description | Used in |
|-------------|-------------|---------|
| `{change_id}` | Change ID being processed | apply_command, acceptance_command, archive_command |
| `{prompt}` | System prompt for agent | All commands |
| `{workspace_dir}` | Worktree path | worktree_command |
| `{repo_root}` | Repository root | worktree_command |

### Hook Placeholders

| Placeholder | Description |
|-------------|-------------|
| `{change_id}` | Current Change ID |
| `{changes_processed}` | Changes processed count |
| `{total_changes}` | Total changes in snapshot |
| `{remaining_changes}` | Remaining changes |
| `{apply_count}` | Apply attempt number |
| `{completed_tasks}` | Completed tasks count |
| `{total_tasks}` | Total tasks count |
| `{status}` | Finish status |
| `{error}` | Error message |

## Configuration Validation

### Check Configuration

Run with dry-run to validate:

```bash
cflx run --dry-run
```

### Test Individual Commands

Test commands manually:

```bash
# Test apply command
claude -p '/openspec:apply test-change Test prompt'

# Test archive command
claude -p '/openspec:archive test-change'
```

## Troubleshooting

### Command Not Found

**Symptom:** `command not found: claude`

**Solution:**
1. Verify agent is installed: `which claude`
2. Update PATH: `export PATH=$PATH:/path/to/agent`
3. Use absolute path in config: `"/usr/local/bin/claude"`

### Configuration Not Loading

**Symptom:** Default behavior instead of custom config

**Solutions:**
1. Check file location: `.cflx.jsonc` in project root
2. Validate JSONC syntax: no trailing commas, valid comments
3. Check for typos in configuration keys
4. Use `--config` to specify custom path

### Placeholder Not Expanding

**Symptom:** Literal `{change_id}` in command output

**Solutions:**
1. Verify placeholder syntax (curly braces)
2. Check placeholder is valid for that command type
3. Ensure quotes around command strings in config

### Hooks Not Executing

**Symptom:** Lifecycle hooks not running

**Solutions:**
1. Check hook name spelling
2. Verify command is executable
3. Test command manually
4. Check `continue_on_failure` setting
5. Review logs for hook errors

## Migration Guide

### From OpenCode to Claude Code

1. Update agent commands:

```jsonc
{
  "analyze_command": "claude -p '{prompt}'",
  "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude -p '/openspec:archive {change_id} {prompt}'"
}
```

2. Update command templates (if using custom commands)

3. Test with dry-run: `cflx run --dry-run`

### From Single to Parallel Mode

1. Configure workspace directory:

```jsonc
{
  "workspace_base_dir": "/path/to/worktrees",
  "max_concurrent_workspaces": 3
}
```

2. Add resolve command:

```jsonc
{
  "resolve_command": "claude -p '{prompt}'"
}
```

3. Test with small concurrency: `cflx run --parallel --max-concurrent 2`

## References

- [Agent Configuration Examples](references/agent-configs.md) - Detailed examples for different agents
- [Hook Configuration](references/hooks-config.md) - Comprehensive hook examples
- [Advanced Settings](references/advanced.md) - Advanced configuration options

## Related

For workflow operations and TUI usage, see the `cflx-workflow` skill.
