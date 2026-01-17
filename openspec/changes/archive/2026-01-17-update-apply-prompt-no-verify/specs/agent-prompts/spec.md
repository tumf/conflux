## MODIFIED Requirements
### Requirement: Apply system prompt MUST include task format guidance

The AI agent's apply prompt (`APPLY_SYSTEM_PROMPT`) MUST include guidance on how to fix tasks.md format issues, keep tasks.md updated as work progresses, and explicitly forbid using `--no-verify` when running git commands.

#### Scenario: AI agent fixes invalid format

**Given:**
- tasks.md contains invalid format (`## 1. Task`, `- Task`, `1. Task`)
- Parser detects 0/0 tasks and apply is executed

**When:**
- AI agent receives the apply prompt

**Then:**
- Prompt includes tasks.md format requirements:
  - Checkboxes are mandatory (`- [ ]`, `- [x]`)
  - Examples of invalid format patterns
  - How to fix each pattern
  - Steps to follow when 0/0 is detected
- Prompt forbids using `--no-verify` in git commands
- AI agent fixes tasks.md following the guidance
- After fix, re-parsing detects correct task count
- After each completed task, the agent updates tasks.md to reflect progress
