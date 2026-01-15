# Capability: Agent Prompts

Defines the content of system prompts passed to AI agents.

---

## ADDED Requirements

### Requirement: Apply system prompt MUST include task format guidance

The AI agent's apply prompt (`APPLY_SYSTEM_PROMPT`) MUST include guidance on how to fix tasks.md format issues.

#### Rationale
When tasks.md has invalid format (missing checkboxes) causing 0/0 tasks detection errors, enable the AI agent to automatically fix the format.

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
- AI agent fixes tasks.md following the guidance
- After fix, re-parsing detects correct task count

**Verification:**
```bash
# 1. Create test change with invalid format
mkdir -p openspec/changes/test-invalid-format
cat > openspec/changes/test-invalid-format/tasks.md <<EOF
# Tasks
## 1. First task
- Second task without checkbox
1. Third numbered task
EOF

# 2. Run apply (AI should auto-fix)
cargo run -- apply test-invalid-format

# 3. Verify fixed format
cat openspec/changes/test-invalid-format/tasks.md
# Expected: Checkboxes added
```
