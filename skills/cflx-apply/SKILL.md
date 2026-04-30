---
name: cflx-apply
description: Implement an approved OpenSpec change autonomously with truthful task tracking. Provides apply-specific guidance for Conflux orchestration. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Apply Executor

Implement an approved OpenSpec change autonomously with task tracking.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

Implement the approved change fully, updating `tasks.md` as progress is made, and providing all AI-executable verification (build/tests/lint) to the extent possible.

## Critical Constraints

- If `openspec/CONSTITUTION.md` exists, read it before implementation and treat it as higher-priority project law than proposal/spec deltas.
- Do not implement changes that violate `openspec/CONSTITUTION.md` unless that constitution is explicitly changed first.
- **NO QUESTIONS** - Make autonomous decisions based on available context
- **NO DEFERRAL** - Do not defer tasks based on difficulty or complexity
- **IMMEDIATE UPDATES** - Update `tasks.md` after EVERY completed task
- **COMPLETE ALL TRUTHFULLY** - A task may be marked `[x]` only when the corresponding repository change and required verification actually exist
- **ESCALATE BLOCKERS** - If implementation is impossible, record an Implementation Blocker for acceptance review
- **NO CHECKLIST-ONLY COMPLETION** - Do not mark implementation tasks complete based only on proposal/spec/tasks edits when the task requires code, tests, or runtime wiring

## Execution Steps

1. **Read Proposal**
   ```bash
   cflx openspec show <change-id>
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

## Truthful Completion Rules

Before changing any task to `[x]`, verify all applicable conditions below are true:

1. The repository contains the required implementation artifact for that task.
   - Code task -> matching `src/`, app, config, or script diff exists.
   - Test task -> matching `tests/` diff exists.
   - Wiring/integration task -> real entrypoint/call-site/config hookup exists.
   - Spec-only task -> it is explicitly documentation/spec work rather than implementation work.
2. The artifact is reachable from the intended flow when the task claims runtime integration.
3. The relevant verification command has been run successfully, or concrete blocker evidence has been recorded.
4. The task description still matches reality. If the task is too broad or ambiguous, refine it before completion.
5. Tasks claiming unit-test coverage are complete only when tests are genuinely unit-scoped and do not rely on real stateful external boundaries.
6. If added tests require real stateful external boundaries, classify them as integration/e2e evidence; do not use them as unit-test completion evidence.
7. Unit-test completion is invalid when the only evidence is integration-style tests that exercise real git/process/filesystem/network/database/timer flows.
8. The planned verification path from proposal/tasks (for example: `unit`, `integration`, `e2e`, `manual`, `benchmark`, `not-testable`) is identified before completion is claimed.
9. The evidence type is consistent with the planned verification type; mismatches MUST be recorded as follow-up work instead of being marked complete.

## Planned Verification Alignment

Apply MUST connect proposal planning and implementation truthfulness:

- Read planned verification ownership from proposal/task context before closing each implementation task.
- Treat `manual`, `benchmark`, and `not-testable` as intentional verification paths when explicitly planned.
- Do not block completion only because unit/integration tests are absent if the planned path is intentionally non-test automation.
- If verification ownership is missing or ambiguous for behavior-changing work, do not silently assume unit tests; add follow-up tasks to clarify planning/enforcement alignment.
- If planned type and evidence type diverge (for example, planned `unit` but only integration-style evidence exists), keep the task open or add explicit mismatch follow-up before completion.
- Record the mismatch with concrete evidence so acceptance can enforce the same verification model.

Evidence type guide:
- Unit evidence: isolated logic tests with mocks/fakes/in-memory doubles only.
- Integration evidence: tests touching real filesystem/process/VCS/network/database/timer or other stateful boundaries.
- Manual evidence: explicit operator/tester procedure and result ownership.
- Benchmark evidence: reproducible performance measurement artifact (command, metric, threshold, result).
- Not-testable evidence: explicit rationale for why automated verification is not feasible plus ownership of ongoing manual/runtime checks.

If verification mismatch is discovered during apply, append unchecked follow-up tasks similar to:

```markdown
## Verification Mismatch Follow-up
- [ ] Extract unit-testable decision logic and add unit-scoped tests for <component>
- [ ] Reclassify coverage as integration/e2e/manual/benchmark when unit ownership is not valid
- [ ] Update proposal/task verification ownership to match actual enforceable evidence
```

Do not mark the parent implementation task complete until mismatch follow-up is represented truthfully in `tasks.md`.

Never mark a task complete based only on any of the following:

- `openspec/` files were updated
- `tasks.md` was normalized
- a proposal was archived or merged
- code was discussed but no runtime/test artifact was added
- a stub placeholder was added where a real execution path was required

## Task Management

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

## Checkbox Rules

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

## Unit Test Boundary Policy

- Unit tests MUST NOT directly depend on real stateful external boundaries.
- Treat the following as unit-test external boundaries: VCS/SCM, network/API, database, real filesystem state, real OS process/CLI tool execution, clock/sleep/timer, and environment-dependent permissions/credentials/OS state.
- For logic-oriented tasks, extract decision logic into helpers/traits/interfaces/pure functions and verify with mocks/fakes/in-memory doubles.
- If a test must exercise real external boundaries, classify it as integration/e2e rather than unit.

## Mock-First Policy

- Mock external dependencies when possible
- Do not block on missing API keys/credentials
- Implement stub/fixture for external services
- For unit-test tasks, isolate decision logic from boundary access and verify with mocks/fakes/in-memory doubles
- Unit-test completion is invalid when tests rely on real stateful external boundaries
- Only truly non-mockable dependencies go to Future Work

## Implementation Blocker Escalation

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
2. Create or update `openspec/changes/<change-id>/REJECTED.md` as an **apply-generated rejection proposal artifact** (not terminal by itself). Include at minimum:
   ```markdown
   # REJECTED

   - change_id: <change-id>
   - reason: <same blocker summary>
   - proposed_by: apply
   ```
3. The blocker section is human-facing and MUST NOT use checkboxes.
4. Output a machine-readable marker at the end of apply output:
   ```text
   IMPLEMENTATION_BLOCKER:
   category: <...>
   tasks_section: "Implementation Blocker #<n>"
   rejection_proposal: openspec/changes/<change-id>/REJECTED.md
   human_action_required: acceptance must confirm rejection proposal
   ```
5. Keep evidence concrete and actionable so acceptance can judge whether loop stop is warranted.

## Apply Completion Criteria

- All tasks marked `[x]` or moved to Future Work (without checkboxes)
- Code compiles/builds successfully
- Tests pass
- Lint passes
- Integration points verified
- Any task that claims implementation, runtime behavior, or entrypoint wiring has corresponding non-OpenSpec evidence in the repo
- Changes that are spec-only MUST leave implementation tasks unchecked or blocked; they must not be represented as completed implementation

**For detailed guidance**, read [references/cflx-apply.md](references/cflx-apply.md).

## Built-in Tools

```bash
# List changes
cflx openspec list

# Show change details
cflx openspec show <id>

# Show JSON output
cflx openspec show <id> --json

# Show deltas only
cflx openspec show <id> --json --deltas-only

# Validate change
cflx openspec validate <id> --strict
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
