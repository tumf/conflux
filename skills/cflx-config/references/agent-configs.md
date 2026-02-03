# Agent Configuration Examples

Detailed configuration examples for different AI coding agents.

## Claude Code

### Basic Configuration

```jsonc
{
  "analyze_command": "claude -p '{prompt}'",
  "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude -p '/openspec:archive {change_id} {prompt}'"
}
```

### Full Configuration

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:accept {change_id} {prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "worktree_command": "claude --dangerously-skip-permissions --verbose -p '/openspec:proposal --worktree {workspace_dir}'",

  "apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。",
  "acceptance_prompt": "",
  "acceptance_prompt_mode": "full",
  "acceptance_max_continues": 10,
  "archive_prompt": ""
}
```

### Command Options

| Option | Description |
|--------|-------------|
| `--dangerously-skip-permissions` | Skip permission prompts (required for automation) |
| `--verbose` | Enable verbose output |
| `--output-format stream-json` | Stream JSON output for real-time progress |
| `-p` | Prompt argument |

## OpenCode

### Basic Configuration

```jsonc
{
  "analyze_command": "opencode run --format json {prompt}",
  "apply_command": "opencode run '/openspec-apply {change_id}'",
  "archive_command": "opencode run '/conflux:archive {change_id}'"
}
```

### Full Configuration

```jsonc
{
  "analyze_command": "opencode run --format json {prompt}",
  "apply_command": "opencode run '/openspec-apply {change_id}'",
  "acceptance_command": "opencode run '/cflx-accept {change_id} {prompt}'",
  "archive_command": "opencode run '/conflux:archive {change_id}'",
  "resolve_command": "opencode run {prompt}",
  "worktree_command": "opencode run '/openspec:proposal --worktree {workspace_dir}'",

  "acceptance_prompt_mode": "context_only"
}
```

### Notes

- OpenCode uses command templates (`.opencode/commands/*.md`)
- `acceptance_prompt_mode` should be `"context_only"` when using command templates
- No need for `--dangerously-skip-permissions` equivalent

## Codex

### Basic Configuration

```jsonc
{
  "analyze_command": "codex run '{prompt}'",
  "apply_command": "codex run '/openspec:apply {change_id} {prompt}'",
  "archive_command": "codex run '/openspec:archive {change_id} {prompt}'"
}
```

### Full Configuration

```jsonc
{
  "analyze_command": "codex run '{prompt}'",
  "apply_command": "codex run '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "codex run '/openspec:accept {change_id} {prompt}'",
  "archive_command": "codex run '/openspec:archive {change_id} {prompt}'",
  "resolve_command": "codex run '{prompt}'",
  "worktree_command": "codex run '/openspec:proposal --worktree {workspace_dir}'",

  "apply_prompt": "Follow OpenSpec workflow strictly",
  "acceptance_prompt": "",
  "archive_prompt": ""
}
```

## Cline

### Basic Configuration

```jsonc
{
  "analyze_command": "cline --headless --prompt '{prompt}'",
  "apply_command": "cline --headless --prompt '/openspec:apply {change_id} {prompt}'",
  "archive_command": "cline --headless --prompt '/openspec:archive {change_id} {prompt}'"
}
```

### Full Configuration

```jsonc
{
  "analyze_command": "cline --headless --prompt '{prompt}'",
  "apply_command": "cline --headless --prompt '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "cline --headless --prompt '/openspec:accept {change_id} {prompt}'",
  "archive_command": "cline --headless --prompt '/openspec:archive {change_id} {prompt}'",
  "resolve_command": "cline --headless --prompt '{prompt}'",

  "acceptance_prompt_mode": "full"
}
```

## Aider

### Basic Configuration

```jsonc
{
  "analyze_command": "aider --yes --message '{prompt}'",
  "apply_command": "aider --yes --message '/openspec:apply {change_id} {prompt}'",
  "archive_command": "aider --yes --message '/openspec:archive {change_id} {prompt}'"
}
```

### Full Configuration

```jsonc
{
  "analyze_command": "aider --yes --message '{prompt}' --output-format json",
  "apply_command": "aider --yes --message '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "aider --yes --message '/openspec:accept {change_id} {prompt}'",
  "archive_command": "aider --yes --message '/openspec:archive {change_id} {prompt}'",
  "resolve_command": "aider --yes --message '{prompt}'",

  "apply_prompt": "Follow OpenSpec tasks strictly",
  "acceptance_prompt": ""
}
```

### Command Options

| Option | Description |
|--------|-------------|
| `--yes` | Auto-confirm all prompts |
| `--message` | Message/prompt argument |
| `--output-format json` | JSON output for parsing |

## Custom Agent Configuration

### Template

```jsonc
{
  "analyze_command": "<agent> <options> '{prompt}'",
  "apply_command": "<agent> <options> '/openspec:apply {change_id} {prompt}'",
  "archive_command": "<agent> <options> '/openspec:archive {change_id} {prompt}'",

  "apply_prompt": "<custom instructions>",
  "acceptance_prompt": "",
  "archive_prompt": ""
}
```

### Requirements

1. **Non-interactive:** Agent must run without user prompts
2. **Exit codes:** 0 for success, non-zero for failure
3. **Output:** Structured output preferred (JSON/stream-json)
4. **Placeholders:** Support `{change_id}` and `{prompt}` expansion

## Multi-Agent Setup

Use different agents for different operations:

```jsonc
{
  "analyze_command": "claude -p '{prompt}'",
  "apply_command": "opencode run '/openspec-apply {change_id}'",
  "acceptance_command": "pytest tests/",
  "archive_command": "claude -p '/openspec:archive {change_id}'",
  "resolve_command": "opencode run {prompt}"
}
```

**Use cases:**
- Fast agent for analyze, powerful agent for apply
- Custom test suite for acceptance
- Specialized conflict resolution agent

## Environment-Specific Configuration

### Development

```jsonc
{
  "analyze_command": "claude -p '{prompt}'",
  "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude -p '/openspec:archive {change_id}'",

  "logging": {
    "suppress_repetitive_debug": false,
    "summary_interval_secs": 30
  }
}
```

### Production

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --output-format stream-json -p '{prompt}'",
  "apply_command": "claude --dangerously-skip-permissions --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --output-format stream-json -p '/openspec:accept {change_id} {prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  },

  "hooks": {
    "post_apply": {
      "command": "cargo test",
      "continue_on_failure": false
    }
  }
}
```

### CI/CD

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --output-format stream-json -p '{prompt}'",
  "apply_command": "claude --dangerously-skip-permissions --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  "acceptance_command": "",  // Disabled in CI

  "max_concurrent_workspaces": 5,

  "hooks": {
    "post_apply": "cargo test --all-features",
    "pre_archive": "cargo clippy -- -D warnings"
  }
}
```

## Troubleshooting

### Agent Not Found

**Symptom:** `command not found: <agent>`

**Solutions:**
1. Install agent: `npm install -g @agent/cli`
2. Add to PATH: `export PATH=$PATH:/path/to/agent`
3. Use absolute path: `"/usr/local/bin/agent"`

### Permission Denied

**Symptom:** `permission denied: <agent>`

**Solutions:**
1. Make executable: `chmod +x /path/to/agent`
2. Check file ownership: `ls -la /path/to/agent`
3. Run with sudo (not recommended)

### Command Hangs

**Symptom:** Agent command never completes

**Solutions:**
1. Add timeout to hook configuration
2. Enable non-interactive mode
3. Check agent logs for blocking prompts
4. Test command manually: `<agent> -p 'test'`

### Output Not Parsed

**Symptom:** No progress updates in TUI

**Solutions:**
1. Enable structured output: `--output-format stream-json`
2. Check stdout/stderr redirection
3. Verify JSON format validity
4. Review agent documentation for output options

## Best Practices

1. **Use absolute paths** for agent binaries in production
2. **Enable non-interactive mode** to prevent blocking
3. **Configure timeouts** for long-running operations
4. **Use structured output** (JSON/stream-json) for parsing
5. **Test commands individually** before orchestration
6. **Version lock agents** in CI/CD environments
7. **Configure appropriate logging** for debugging
8. **Use environment variables** for sensitive data
