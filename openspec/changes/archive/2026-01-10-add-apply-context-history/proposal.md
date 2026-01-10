# Add Apply Context History

## Summary

Enhance apply command continuity by including previous apply attempt results in subsequent prompts. This allows AI agents to learn from previous execution context and maintain processing continuity across retries.

## Background

Currently, each apply command is executed with a static prompt:
```
'/openspec:apply {change_id} DEFAULT_APPLY_PROMPT'
```

When a change requires multiple apply attempts (due to partial completion or errors), the agent starts fresh each time without knowledge of previous attempts. This can lead to:
- Repeated mistakes
- Loss of context about what was already tried
- Inefficient retry cycles

## Proposed Solution

Add context history tracking at the orchestrator level (memory-only) and inject the agent's last message (summary) into the prompt for subsequent attempts:

```
'/openspec:apply {change_id} DEFAULT_APPLY_PROMPT

<last_apply attempt="1">
Implemented authentication middleware in auth.rs. Found type mismatch issue at line 42 that needs to be resolved. Tasks 1.1 and 1.2 completed, task 1.3 in progress.
</last_apply>
'
```

The `openspec:apply` skill returns a summary message when it completes. This summary is captured and provided to the next apply attempt, allowing the agent to maintain continuity.

## Scope

### In Scope
- Capture the agent's final summary message from each apply attempt
- Store attempt history per change_id in memory
- Include previous attempt summaries in `{prompt}` placeholder for 2nd+ attempts
- Clear history when change is archived

### Out of Scope
- Persistent storage (file-based history)
- Full log capture (only summary)
- Archive command history (apply only)

## Key Design Decisions

1. **Memory-only storage**: History lost on session end, acceptable for single-run scenarios
2. **Agent summary capture**: The `openspec:apply` skill returns a summary message; we capture this as the history content
3. **Placeholder injection**: Append to existing `{prompt}` content, not replace

## Dependencies

None - builds on existing prompt expansion infrastructure
