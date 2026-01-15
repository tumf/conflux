# Proposal: Add tasks.md Format Correction Guidance to AI Agent Prompt

## Why

Currently, when tasks.md has an invalid format (missing checkboxes), the parser returns 0/0 tasks and the apply operation fails. The `fix-parallel-merge-completed-status` change demonstrated this issue with invalid tasks.md format. Error messages alone don't clarify how to fix the format, and this issue can occur in both Sequential and Parallel apply modes.

To resolve this, we need to enable AI agents to automatically detect and fix invalid task formats by adding clear guidance to the apply system prompt.

## What Changes

Add format correction guidance to the `APPLY_SYSTEM_PROMPT` constant in `src/agent.rs`.

**Invalid format example:**
```markdown
## 1. Task title
Description...

- Task without checkbox
1. Numbered task
```

**Correct format:**
```markdown
- [ ] 1. Task title
Description...

- [ ] Task without checkbox
1. [ ] Numbered task
```

### Content to Add

```rust
Tasks format requirements:
- All tasks MUST have checkboxes: `- [ ]` or `- [x]`
- Invalid formats that need fixing:
  * `## N. Task` → Convert to `- [ ] N. Task`
  * `- Task` → Convert to `- [ ] Task`
  * `1. Task` → Convert to `1. [ ] Task`
- If you encounter 0/0 tasks detected, check and fix tasks.md format first
```

### Operation Flow

#### Sequential apply
```
Run apply
  ↓
Detect 0/0 tasks (parse_file() fails)
  ↓
Launch AI agent
  ↓
Fix tasks.md following prompt guidance
  ↓
Re-parse succeeds → Continue apply
```

#### Parallel run
```
Start parallel execution
  ↓
Run apply for each change
  ↓
Detect 0/0 tasks
  ↓
AI agent auto-fixes following guidance
  ↓
Continue apply
```

## Impact

### Changes Required
- `src/agent.rs` - `APPLY_SYSTEM_PROMPT` constant only

### Unaffected Areas
- `src/task_parser.rs` - No parser logic changes needed
- `src/execution/apply.rs` - No apply execution logic changes needed
- `src/parallel/executor.rs` - No parallel execution logic changes needed

### Backward Compatibility
- ✅ No impact on existing behavior (only adding guidance to prompt)
- ✅ No configuration file changes required
- ✅ No impact on existing tests.md files

## Alternatives

| Approach | Pros | Cons |
|----------|------|------|
| **Add Prompt Guidance (Proposed)** | • Simple implementation<br>• No existing code changes<br>• Works for both modes | • Depends on AI interpretation |
| Implement Auto-fix Code | • Guaranteed fix<br>• No AI dependency | • Complex implementation<br>• Edge case handling |
| Add Validation Command | • Can detect issues early | • Requires manual execution<br>• No auto-fix |

## Success Criteria

1. ✅ Format guidance added to `src/agent.rs`
2. ✅ When applying a change with invalid tasks.md, AI agent auto-fixes it
3. ✅ Works in both Sequential and Parallel modes
4. ✅ All existing tests pass

## Risks

### Risks
- AI agent might ignore the guidance
- Possible incorrect fixes with complex formats (nested lists, code blocks)

### Mitigation
- Write guidance clearly and specifically
- Consider adding validation command (`validate`) in the future
