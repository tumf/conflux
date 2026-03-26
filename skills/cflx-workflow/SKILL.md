---
name: cflx-workflow
description: Execute Conflux workflow operations autonomously without user interaction. Provides three operations - apply (implement approved changes), accept (verify implementation), and archive (finalize deployed changes). Called by Conflux orchestration system. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Workflow Executor

Execute Conflux workflow operations autonomously. Called by orchestration system, not for direct human use.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Operation Modes

This skill supports three operations, determined by the orchestrator's invocation:

1. **apply** - Implement an approved change
2. **accept** - Verify implementation against specs
3. **archive** - Finalize a deployed change

## Operation Selection

The orchestrator specifies the operation. Parse the invocation to determine:

- If change ID with "apply" or "implement" context → Execute Apply
- If "accept" or "review" context → Execute Accept
- If "archive" context → Execute Archive

## Operation 1: Apply (Implementation)

**Purpose**: Implement an approved change autonomously with task tracking.

**CRITICAL CONSTRAINTS**:
- **NO QUESTIONS** - Make autonomous decisions based on available context
- **NO DEFERRAL** - Do not defer tasks based on difficulty or complexity
- **IMMEDIATE UPDATES** - Update `tasks.md` after EVERY completed task
- **COMPLETE ALL TRUTHFULLY** - A task may be marked `[x]` only when the corresponding repository change and required verification actually exist
- **ESCALATE BLOCKERS** - If implementation is impossible, record an Implementation Blocker for acceptance review
- **NO CHECKLIST-ONLY COMPLETION** - Do not mark implementation tasks complete based only on proposal/spec/tasks edits when the task requires code, tests, or runtime wiring

### Execution Steps

1. **Read Proposal**
   ```bash
   python3 "<SKILL_ROOT>/scripts/cflx.py" show <change-id>
   ```
   - Read `openspec/changes/<id>/proposal.md`
   - Read `openspec/changes/<id>/design.md` (if exists)
   - Read `openspec/changes/<id>/tasks.md`

2. **Work Through Tasks Sequentially**
    - Start with first uncompleted task
    - Implement the change
    - Run verification (build/test/lint)
    - Mark task as `[x]` in `tasks.md` immediately after the implementation and verification evidence exist
    - Proceed to next task

3. **Handle Ambiguity Autonomously**
   - Use existing code patterns as reference
   - Make reasonable assumptions
   - Document decisions in code comments
   - Prefer simpler solutions

4. **Update Progress Continuously**
   - Update `tasks.md` after each task
   - Never batch updates
   - Keep progress visible

5. **Verify Completion**
    - Ensure all tasks are `[x]` or in Future Work
    - Run final validation
    - Confirm integration points

### Truthful Completion Rules

Before changing any task to `[x]`, verify all applicable conditions below are true:

1. The repository contains the required implementation artifact for that task.
   - Code task -> matching `src/`, app, config, or script diff exists.
   - Test task -> matching `tests/` diff exists.
   - Wiring/integration task -> real entrypoint/call-site/config hookup exists.
   - Spec-only task -> it is explicitly documentation/spec work rather than implementation work.
2. The artifact is reachable from the intended flow when the task claims runtime integration.
3. The relevant verification command has been run successfully, or concrete blocker evidence has been recorded.
4. The task description still matches reality. If the task is too broad or ambiguous, refine it before completion.

Never mark a task complete based only on any of the following:

- `openspec/` files were updated
- `tasks.md` was normalized
- a proposal was archived or merged
- code was discussed but no runtime/test artifact was added
- a stub placeholder was added where a real execution path was required

### Task Management

**Move to Future Work ONLY if**:
1. Requires human decision-making or judgment
2. Requires external system access outside repository
3. Requires long-wait verification (>1 day)
4. Already marked with '(future work)'

**Do NOT move to Future Work**:
- Difficult or complex tasks (agent must attempt)
- Tests (unit/integration/e2e)
- Linting/formatting
- Documentation updates
- Any automatable task

### Checkbox Rules

**Active sections**: Must have checkboxes `- [ ]` or `- [x]`

**Excluded sections** (Future Work, Out of Scope, Notes): Must NOT have checkboxes

```markdown
## Implementation Tasks
- [x] Completed task
- [ ] Pending task

## Future Work
- Manual verification required
- External deployment needed
```

### Mock-First Policy

- Mock external dependencies when possible
- Do not block on missing API keys/credentials
- Implement stub/fixture for external services
- Only truly non-mockable dependencies go to Future Work

### Implementation Blocker Escalation

If apply determines the change is currently impossible to implement (for example: spec contradiction, non-mockable external limitation, or policy constraint), do not loop blindly.

1. Add a new section to `openspec/changes/<change-id>/tasks.md`:
   ```markdown
   ## Implementation Blocker #<n>
   - category: <spec_contradiction|external_non_mockable|policy_constraint|other>
   - summary: <one-line human-facing blocker summary>
   - evidence:
      - <file/path:line or concrete command output>
   - impact: <what cannot be completed>
   - unblock_actions:
      - <specific follow-up action 1>
      - <specific follow-up action 2>
   - owner: <team_or_role>
   - decision_due: <YYYY-MM-DD>
   ```
2. The blocker section is human-facing and MUST NOT use checkboxes.
3. Output a machine-readable marker at the end of apply output:
   ```text
   IMPLEMENTATION_BLOCKER:
   category: <...>
   tasks_section: "Implementation Blocker #<n>"
   human_action_required: see openspec/changes/<change-id>/tasks.md#implementation-blocker-<n>
   ```
4. Keep evidence concrete and actionable so acceptance can judge whether loop stop is warranted.

### Apply Completion Criteria

- All tasks marked `[x]` or moved to Future Work (without checkboxes)
- Code compiles/builds successfully
- Tests pass
- Lint passes
- Integration points verified
- Any task that claims implementation, runtime behavior, or entrypoint wiring has corresponding non-OpenSpec evidence in the repo
- Changes that are spec-only MUST leave implementation tasks unchecked or blocked; they must not be represented as completed implementation

**For detailed guidance**, read [references/cflx-apply.md](references/cflx-apply.md).

## Operation 2: Accept (Acceptance Review)

**Purpose**: Verify implementation meets specifications with automated checks.

**CRITICAL**: Output exactly ONE verdict at the end.

### Spec-Only Change Detection

Before running checks, read `proposal.md` and detect the `Change Type` field:
- If `Change Type: spec-only` → apply **Spec-Only Acceptance** path (checks 4–7 are replaced)
- Otherwise → apply the standard implementation acceptance path

**Spec-Only Acceptance** (replaces checks 4–7 for spec-only changes):
- **Archive simulation**: For each spec delta in `openspec/changes/<id>/specs/<capability>/spec.md`, simulate whether archiving would produce a net change to `openspec/specs/<capability>/spec.md`.
  - FAIL if delta would be a no-op (e.g., MODIFIED target not found in canonical spec, or delta is empty).
  - FAIL if delta contains only `MODIFIED`/`REMOVED` sections and the canonical target is missing.
- **Spec tasks**: All `## Specification Tasks` entries must be `[x]` or in Future Work. Absence of runtime code is NOT a failure.
- **No runtime evidence required**: Do NOT fail because source files, tests, or CLI wiring are absent. The change type is spec-only by design.
- **Evidence citation**: cite the spec delta file path and expected canonical promotion target.

### Required Checks

1. **Git Working Tree Clean**
   ```bash
   git status --porcelain
   ```
   - Must output empty (no uncommitted changes)
   - If dirty, output FAIL with all changed files

2. **Task Completion**
    - All tasks marked `[x]` or in Future Work section
    - No checkboxes in excluded sections
    - Reject any task marked `[x]` without corresponding repo evidence
    - For spec-only: `openspec/` edits ARE the implementation artifact; no runtime evidence required

3. **Spec Matching**
   - Implementation matches specification in `specs/`
   - All scenarios are satisfied

4. **Integration Check** *(skip for spec-only)*
    - Feature is executed in real flow
    - Called from CLI/TUI/API as specified

5. **Dead Code Check** *(skip for spec-only)*
   - All implemented code is invoked
   - No orphan functions/classes

6. **No Stubbed Runtime Check** *(skip for spec-only)*
   - Real execution path must not use mock/stub/fake/placeholder

7. **Regression Check**
   - Existing features still work
   - No unintended side effects

8. **Evidence Citation**
    - Cite file path + function/method for integration
    - For spec-only: cite spec delta path + canonical promotion target

9. **Checklist Truthfulness Check**
   - FAIL if `tasks.md` claims completion but the corresponding code/tests/entrypoints do not exist
   - FAIL if a change was archived/spec-promoted while implementation tasks were marked complete without repository evidence
   - FAIL if the only evidence for an *implementation* task is `openspec/` edits
   - Exception: for `spec-only` changes, `openspec/` spec delta edits ARE the expected artifact

### Output Format

Output exactly ONE verdict marker at the end.

**CRITICAL formatting rule**: The marker line (e.g. `ACCEPTANCE: PASS`) MUST be on its own line with NOTHING else on that line — no trailing text, no inline explanation. The orchestrator parses this marker by line; any text appended to the same line (e.g. `ACCEPTANCE: PASSAll criteria verified`) will break detection.

**PASS**:
```
ACCEPTANCE: PASS
```

**FAIL**:
```
ACCEPTANCE: FAIL

FINDINGS:
1. [file:line] Description of issue
2. [file:line] Description of issue
...
```
Then update `tasks.md` with:
```markdown
## Acceptance #N Failure Follow-up

- [ ] Fix issue 1
- [ ] Fix issue 2
```

**CONTINUE** (only if verification incomplete):
```
ACCEPTANCE: CONTINUE
```

**BLOCKED** (when blocker escalation is valid):
```
ACCEPTANCE: BLOCKED

BLOCKER:
- category: <...>
- reason: <short rationale>
- evidence: <file/path:line or command evidence>

Recommended:
- summary: <one-line human-facing blocker summary>
- unblock_actions:
  - <specific follow-up action 1>
  - <specific follow-up action 2>
```

### Accept Rules

- Each finding must include concrete evidence (file path, function, line)
- Each finding must be actionable by AI agent
- Missing secrets MUST NOT cause CONTINUE if mocking is possible
- Dirty working tree is always FAIL
- `ACCEPTANCE: BLOCKED` is allowed only when a valid `Implementation Blocker #<n>` exists with concrete evidence and unblock actions
- If blocker data is weak, speculative, or fixable within repo scope, return FAIL instead of BLOCKED

**For detailed guidance**, read [references/cflx-accept.md](references/cflx-accept.md).

## Operation 3: Archive

**Purpose**: Archive deployed change and update canonical specs.

### Execution Steps

1. **Identify Change ID**
   - From orchestrator invocation
   - Or from context (must be unambiguous)

2. **Validate Change Status**
   ```bash
   python3 "<SKILL_ROOT>/scripts/cflx.py" list
   python3 "<SKILL_ROOT>/scripts/cflx.py" show <id>
   ```
   - Ensure change exists
   - Ensure not already archived
   - Ensure ready for archive

3. **Run Archive**
   ```bash
   python3 "<SKILL_ROOT>/scripts/cflx.py" archive <id> --yes
   ```
   - Use `--skip-specs` only for tooling-only changes

4. **Verify Results**
   - Confirm moved to `changes/archive/`
   - Confirm specs updated
   ```bash
   python3 "<SKILL_ROOT>/scripts/cflx.py" validate --strict
   ```

### Archive Completion Criteria

- Change moved to `openspec/changes/archive/<id>/`
- Canonical specs updated (unless `--skip-specs`)
- Validation passes with `--strict`

**For detailed guidance**, read [references/cflx-archive.md](references/cflx-archive.md).

## Built-in Tools

```bash
# List changes
python3 "<SKILL_ROOT>/scripts/cflx.py" list

# List specs
python3 "<SKILL_ROOT>/scripts/cflx.py" list --specs

# Show change details
python3 "<SKILL_ROOT>/scripts/cflx.py" show <id>

# Show JSON output
python3 "<SKILL_ROOT>/scripts/cflx.py" show <id> --json

# Show deltas only
python3 "<SKILL_ROOT>/scripts/cflx.py" show <id> --json --deltas-only

# Validate change
python3 "<SKILL_ROOT>/scripts/cflx.py" validate <id> --strict

# Validate all
python3 "<SKILL_ROOT>/scripts/cflx.py" validate --strict

# Archive change
python3 "<SKILL_ROOT>/scripts/cflx.py" archive <id> --yes

# Archive without spec updates
python3 "<SKILL_ROOT>/scripts/cflx.py" archive <id> --yes --skip-specs
```

## Autonomous Decision Framework

When facing ambiguous situations, follow this priority:

1. **Existing patterns** - Follow patterns in the codebase
2. **Specification** - Refer to spec deltas and scenarios
3. **Simplicity** - Choose simpler implementation
4. **Documentation** - Document decision in code comments

**Never**:
- Ask user for clarification
- Stop and wait for input
- Leave tasks incomplete due to uncertainty

## Task Format Requirements

**Valid**:
```markdown
- [ ] Task description
- [x] Completed task
1. [ ] Numbered task
```

**Invalid** (must fix):
```markdown
## N. Task              → - [ ] N. Task
- Task                 → - [ ] Task
1. Task                → 1. [ ] Task
```

If `0/0 tasks detected`, fix format first.

## Error Handling

### Validation Failure
1. Parse error messages
2. Fix identified issues
3. Re-run validation
4. Repeat until passing

### Build/Test Failure
1. Analyze error output
2. Fix code issues
3. Re-run verification
4. Update tasks on success

### Incomplete Information
1. Make reasonable assumption
2. Implement based on assumption
3. Document assumption in code
4. Continue with next task

## Reference Files

Detailed operation guides:
- **[references/cflx-apply.md](references/cflx-apply.md)** - Apply operation details
- **[references/cflx-accept.md](references/cflx-accept.md)** - Accept operation details
- **[references/cflx-archive.md](references/cflx-archive.md)** - Archive operation details

## Summary

| Operation | Trigger | Output | Constraints |
|-----------|---------|--------|-------------|
| Apply | "apply <id>" | Completed tasks + code | No questions, update immediately |
| Accept | "accept" | PASS/FAIL/CONTINUE/BLOCKED | Output once, cite evidence |
| Archive | "archive <id>" | Archived change | Validate before/after |

**REMEMBER**: This skill operates autonomously. Never ask questions. Make decisions based on available context.
