# Change: Add Hardcoded System Prompt for Apply Command

## Why

The current `apply_prompt` configuration is user-customizable, but certain instructions should always be enforced during apply operations regardless of user configuration. These non-negotiable instructions ensure proper task management.

## What Changes

1. Add a hardcoded constant `APPLY_SYSTEM_PROMPT` in `src/agent.rs`
2. Modify prompt construction: `{prompt}` = `apply_prompt` (user config) + `APPLY_SYSTEM_PROMPT` (hardcoded) + `history_context` (if any)
3. Remove Japanese default from `apply_prompt` in templates (set to empty string)
4. System prompt content (English):
   - "Remove out-of-scope tasks."
   - "Remove tasks that wait for or require user action."

## Impact

- Affected specs: `configuration`
- Affected files: `src/agent.rs`, `src/templates.rs`
- Backward compatible: `apply_prompt` remains customizable, system prompt is always appended
