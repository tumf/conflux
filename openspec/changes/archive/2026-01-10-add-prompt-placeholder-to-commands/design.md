# Design: Add {prompt} Placeholder to Commands

## Architecture Overview

This change extends the existing placeholder system to support `{prompt}` in `apply_command` and `archive_command`, in addition to the existing `{change_id}` placeholder.

## Configuration Schema Changes

### New Fields

```rust
pub struct OrchestratorConfig {
    // Existing fields...
    pub apply_command: Option<String>,
    pub archive_command: Option<String>,
    pub analyze_command: Option<String>,

    // NEW: Prompt configuration
    pub apply_prompt: Option<String>,
    pub archive_prompt: Option<String>,
    // ...
}
```

### Default Values

```rust
/// Default prompt for apply command - instructs agent to clean up out-of-scope tasks
pub const DEFAULT_APPLY_PROMPT: &str =
    "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。";

/// Default prompt for archive command - empty (no additional instructions)
pub const DEFAULT_ARCHIVE_PROMPT: &str = "";
```

## Command Expansion Flow

### Current Flow

```
apply_command: "claude apply {change_id}"
                       ↓
expand_change_id(template, "fix-bug")
                       ↓
"claude apply fix-bug"
```

### New Flow

```
apply_command: "claude apply {change_id} {prompt}"
                       ↓
expand_change_id(template, "fix-bug")
                       ↓
"claude apply fix-bug {prompt}"
                       ↓
expand_prompt(template, "Remove out-of-scope tasks")
                       ↓
"claude apply fix-bug Remove out-of-scope tasks"
```

## Implementation Details

### AgentRunner Changes

The `run_apply_streaming()` and related methods need to be updated:

```rust
pub async fn run_apply_streaming(
    &self,
    change_id: &str,
) -> Result<(Child, mpsc::Receiver<OutputLine>)> {
    let template = self.config.get_apply_command();
    let prompt = self.config.get_apply_prompt();

    // Expand both placeholders
    let command = OrchestratorConfig::expand_change_id(template, change_id);
    let command = OrchestratorConfig::expand_prompt(&command, prompt);

    info!("Running apply command: {}", command);
    self.execute_shell_command_streaming(&command).await
}
```

### Template Updates

Templates should include both placeholders:

```jsonc
{
  "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude -p '/openspec:archive {change_id} {prompt}'",
  "apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。",
  "archive_prompt": ""
}
```

## Backward Compatibility

Commands without `{prompt}` placeholder will continue to work:
- `expand_prompt()` simply replaces `{prompt}` with the value
- If `{prompt}` doesn't exist in the template, no replacement occurs
- Existing configurations remain functional

## JSONC Example

```jsonc
{
  // Command templates with both placeholders
  "apply_command": "claude --dangerously-skip-permissions -p '/openspec:apply {change_id} {prompt}'",
  "archive_command": "claude --dangerously-skip-permissions -p '/openspec:archive {change_id} {prompt}'",

  // System prompts (optional - defaults will be used if not specified)
  "apply_prompt": "スコープ外タスクは削除せよ。ユーザを待つもしくはユーザによるタスクは削除せよ。",
  "archive_prompt": ""
}
```

## Trade-offs

### Pros
- Enables system-level control over agent behavior
- Configurable per-project or globally
- Backward compatible with existing configurations

### Cons
- Adds complexity to the configuration schema
- Two more configuration options to document and maintain

### Alternatives Considered

1. **Hardcoded prompts**: Rejected - not flexible enough for different use cases
2. **Environment variables**: Rejected - less discoverable than config options
3. **Separate config file for prompts**: Rejected - over-engineering for this use case
