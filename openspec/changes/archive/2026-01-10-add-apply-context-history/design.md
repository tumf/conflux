# Design: Apply Context History

## Architecture Overview

```
┌──────────────────────┐
│   AgentRunner        │
│  ┌────────────────┐  │
│  │ ApplyHistory   │  │  HashMap<String, Vec<ApplyAttempt>>
│  │ (per change)   │  │
│  └────────────────┘  │
│                      │
│  run_apply()         │
│    ↓                 │
│  build_prompt()      │  Combines: base_prompt + history context
│    ↓                 │
│  execute_command()   │
│    ↓                 │
│  record_attempt()    │  Captures: status, duration, error
└──────────────────────┘
```

## Data Structures

### ApplyAttempt

```rust
/// Summary of a single apply attempt
#[derive(Debug, Clone)]
pub struct ApplyAttempt {
    /// Attempt number (1-based)
    pub attempt: u32,
    /// Whether the attempt succeeded
    pub success: bool,
    /// Duration of the attempt
    pub duration: Duration,
    /// Error message if failed (None if success)
    pub error: Option<String>,
    /// Exit code if available
    pub exit_code: Option<i32>,
}
```

### ApplyHistory

```rust
/// Tracks apply attempts per change
pub struct ApplyHistory {
    /// Map of change_id to list of attempts
    attempts: HashMap<String, Vec<ApplyAttempt>>,
}

impl ApplyHistory {
    pub fn new() -> Self;

    /// Record a new attempt for a change
    pub fn record(&mut self, change_id: &str, attempt: ApplyAttempt);

    /// Get all attempts for a change
    pub fn get(&self, change_id: &str) -> Option<&[ApplyAttempt]>;

    /// Get the last attempt for a change
    pub fn last(&self, change_id: &str) -> Option<&ApplyAttempt>;

    /// Get attempt count for a change
    pub fn count(&self, change_id: &str) -> u32;

    /// Clear history for a change (call on archive)
    pub fn clear(&mut self, change_id: &str);

    /// Format history as context string for prompt injection
    pub fn format_context(&self, change_id: &str) -> String;
}
```

## Prompt Format

The context is appended to the base prompt:

```
{base_prompt}

<last_apply attempt="1">
status: failed
duration: 45s
error: Type error in auth.rs:42 - expected String, found i32
exit_code: 1
</last_apply>
```

For multiple previous attempts:

```
{base_prompt}

<last_apply attempt="1">
status: failed
duration: 30s
error: Missing dependency: serde
exit_code: 1
</last_apply>

<last_apply attempt="2">
status: failed
duration: 45s
error: Type error in auth.rs:42
exit_code: 1
</last_apply>
```

## Integration Points

### AgentRunner Changes

```rust
pub struct AgentRunner {
    config: OrchestratorConfig,
    apply_history: ApplyHistory,  // NEW: Add history tracking
}

impl AgentRunner {
    /// Run apply command with history context
    pub async fn run_apply(&mut self, change_id: &str) -> Result<ExitStatus> {
        let start = Instant::now();

        // Build prompt with history context
        let base_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);
        let full_prompt = if history_context.is_empty() {
            base_prompt.to_string()
        } else {
            format!("{}\n\n{}", base_prompt, history_context)
        };

        let template = self.config.get_apply_command();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);

        // Execute command
        let status = self.execute_shell_command(&command).await?;
        let duration = start.elapsed();

        // Record attempt
        let attempt = ApplyAttempt {
            attempt: self.apply_history.count(change_id) + 1,
            success: status.success(),
            duration,
            error: if status.success() { None } else {
                Some(format!("Exit code: {:?}", status.code()))
            },
            exit_code: status.code(),
        };
        self.apply_history.record(change_id, attempt);

        Ok(status)
    }

    /// Clear history for archived change
    pub fn clear_apply_history(&mut self, change_id: &str) {
        self.apply_history.clear(change_id);
    }
}
```

### Orchestrator Integration

In `orchestrator.rs`, call `clear_apply_history()` after successful archive:

```rust
match self.archive_change(&next).await {
    Ok(_) => {
        // Clear apply history for archived change
        self.agent.clear_apply_history(&next.id);
        // ... existing code
    }
    // ...
}
```

## Error Capture Enhancement

For more useful error context, capture stderr from failed commands:

```rust
// In execute_shell_command, capture output even on failure
let output = Command::new(&shell)
    .arg("-l")
    .arg("-c")
    .arg(command)
    .output()
    .await?;

if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Extract last few lines or key error patterns
    let error_summary = extract_error_summary(&stderr);
    // Use this in ApplyAttempt.error
}
```

## Configuration (Future Enhancement)

Could add to `.openspec-orchestrator.jsonc`:

```jsonc
{
  // Maximum number of previous attempts to include in context
  "apply_history_max_attempts": 3,

  // Whether to include history in prompts (default: true)
  "apply_history_enabled": true
}
```

## Testing Strategy

1. **Unit tests for ApplyHistory**
   - Record and retrieve attempts
   - Format context correctly
   - Clear on archive

2. **Integration tests for AgentRunner**
   - First apply has no history context
   - Second apply includes previous attempt
   - Multiple attempts accumulate

3. **End-to-end test**
   - Verify prompt contains history on retry
