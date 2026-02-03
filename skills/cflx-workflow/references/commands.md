# Agent Command Templates

Configuration for AI agent commands used in the orchestration workflow.

## Command Configuration

Agent commands are configured in `.cflx.jsonc`:

```jsonc
{
  "analyze_command": "claude -p '{prompt}'",
  "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "claude -p '/openspec:accept {change_id} {prompt}'",
  "archive_command": "claude -p '/openspec:archive {change_id} {prompt}'",
  "resolve_command": "claude -p '{prompt}'",
  "worktree_command": "claude -p '/openspec:proposal --worktree {workspace_dir}'"
}
```

## Available Commands

### analyze_command

Analyze dependencies and select next change to process.

**Placeholders:**
- `{prompt}`: System prompt for dependency analysis

**Example:**

```jsonc
"analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'"
```

### apply_command

Apply a change by executing tasks from `tasks.md`.

**Placeholders:**
- `{change_id}`: The change ID being processed
- `{prompt}`: System prompt for apply operation

**Example:**

```jsonc
"apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'"
```

### acceptance_command

Run acceptance tests after apply completes.

**Placeholders:**
- `{change_id}`: The change ID being tested
- `{prompt}`: System prompt for acceptance testing

**Example:**

```jsonc
"acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:accept {change_id} {prompt}'"
```

**Skip acceptance:** Set to empty string to disable:

```jsonc
"acceptance_command": ""
```

### archive_command

Archive completed changes to `openspec/specs/`.

**Placeholders:**
- `{change_id}`: The change ID being archived
- `{prompt}`: System prompt for archive operation

**Example:**

```jsonc
"archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'"
```

### resolve_command

Resolve merge conflicts in parallel mode.

**Placeholders:**
- `{prompt}`: System prompt for conflict resolution

**Example:**

```jsonc
"resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'"
```

### worktree_command

Create a proposal worktree from TUI (`+` key).

**Placeholders:**
- `{workspace_dir}`: New worktree path for proposals
- `{repo_root}`: Repository root path

**Example:**

```jsonc
"worktree_command": "claude --dangerously-skip-permissions --verbose -p '/openspec:proposal --worktree {workspace_dir}'"
```

## System Prompts

Customize prompts injected into `{prompt}` placeholder:

### apply_prompt

Prompt for apply command. Default includes path context:

```jsonc
"apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。"
```

**Default value:**

```
<system-context>
IMPORTANT: You are running in the repository root directory.
The change you are working on is located at: openspec/changes/{change_id}/
All file paths should be relative to the repository root.
</system-context>
```

### acceptance_prompt

Prompt for acceptance command:

```jsonc
"acceptance_prompt": ""
```

**Prompt modes:**

Control how acceptance prompt is constructed:

```jsonc
"acceptance_prompt_mode": "full"  // or "context_only"
```

- `"full"`: Include hardcoded acceptance system prompt + diff/history context (default)
- `"context_only"`: Only include change metadata + diff/history context

Use `"context_only"` when your acceptance command template has fixed instructions.

**Retry configuration:**

```jsonc
"acceptance_max_continues": 10
```

Maximum number of `CONTINUE` retries before treating as `FAIL` (default: 10).

### archive_prompt

Prompt for archive command:

```jsonc
"archive_prompt": ""
```

## Agent Compatibility

### Claude Code

```jsonc
{
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:accept {change_id} {prompt}'",
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'"
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

### OPENSPEC_CMD

Override the OpenSpec command:

```bash
# Use a custom openspec installation
export OPENSPEC_CMD="/usr/local/bin/openspec"
cflx

# Use a specific version via npx
export OPENSPEC_CMD="npx @fission-ai/openspec@1.2.3"
cflx
```

**Default:** `npx @fission-ai/openspec@latest`

## Best Practices

1. **Use stream-json output format** for real-time progress updates
2. **Skip permission prompts** in automated environments (`--dangerously-skip-permissions`)
3. **Configure acceptance_prompt_mode** based on command template structure
4. **Set appropriate timeout values** for long-running commands
5. **Test commands individually** before using in orchestration
