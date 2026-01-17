# agent-prompts Specification

## Purpose

This specification defines the behavior and constraints for AI agent system prompts, particularly the apply prompt (`APPLY_SYSTEM_PROMPT`), to ensure reliable and autonomous task execution.
## Requirements
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

### Requirement: Apply system prompt MUST enforce non-interactive iteration

The apply system prompt (`APPLY_SYSTEM_PROMPT`) MUST explicitly state that the agent cannot ask questions to the user and must continue working until MaxIteration is reached, making autonomous decisions under operational constraints.

#### Scenario: Continue iteration without asking questions

**Given:**
- apply execution encounters an uncertain decision point

**When:**
- apply agent processes tasks

**Then:**
- Agent does not ask questions to the user
- Agent makes best autonomous decision and proceeds
- Agent continues iteration until MaxIteration is reached

### Requirement: Future Work restrictions MUST be strictly enforced

The apply system prompt MUST explicitly prohibit moving tasks to Future Work based on difficulty, regression risk, or need for additional testing (MUST NOT).

#### Scenario: Prohibit moving tasks to Future Work except for pre-marked items

**Given:**
- tasks.md contains high-difficulty items

**When:**
- apply agent determines implementation approach

**Then:**
- Agent does NOT move tasks to Future Work based solely on difficulty
- Agent treats only tasks already marked with `(future work)` as Future Work

