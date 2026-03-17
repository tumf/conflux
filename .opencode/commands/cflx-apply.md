---
agent: build
description: Implement an approved OpenSpec change and keep tasks in sync.
---

change_id: $1

The user has requested to implement the change `$1` proposal. Find the change proposal and follow the instructions below.

$ARGUMENTS

**CRITICAL**: You CANNOT ask questions to the user during apply operations. If anything is unclear or ambiguous, make your best autonomous decision based on available context (proposal.md, design.md, tasks.md, existing code patterns). Document your decisions in implementation comments if needed.


**Goal**: Achieve 100% task completion (all tasks in `openspec/chagens/{change_id}/tasks.md` marked as `- [x]` or moved to Future Work). Implement the approved change fully; update the `tasks.md` as progress is made; and provide all AI-executable verification (build/tests/lint) to the extent possible.
**Non-Goal**: Archiving the change or running any archive command; human-only steps (manual verification, visual checks, approvals); long-wait tests; production deployment or production testing.

**MUST**: The files under `openspec/changes/*` (tasks.md, design.md, proposal.md) must be written in Japanese.

<!-- OPENSPEC:START -->
**Guardrails**
- Favor straightforward, minimal implementations first and add complexity only when it is requested or clearly required.
- Keep changes tightly scoped to the requested outcome.
- Do not run `openspec apply` (the command does not exist).
- Do not run `/cflx-archive`, `openspec archive`, or any archive command during apply. Archiving is handled by the orchestrator.
- Refer to `openspec/AGENTS.md` (located inside the `openspec/` directory—run `ls openspec` or `npx @fission-ai/openspec@latest update` if you don't see it) if you need additional OpenSpec conventions or clarifications.

**Steps**
Track these steps as TODOs and complete them one by one.
1. Read `changes/<id>/proposal.md`, `design.md` (if present), and `tasks.md` to confirm scope and acceptance criteria.
2. Work through tasks sequentially, keeping edits minimal and focused on the requested change.
3. While implementing, update `tasks.md` frequently so progress stays in sync (e.g., after each meaningful subtask).
4. Confirm completion before updating statuses—make sure every item in `tasks.md` is finished, including integration/entry-point wiring.
5. If a task adds new functionality, verify it is reachable from an execution path (call site, CLI/TUI flow, or config entry) before marking complete.
6. Update the checklist after all work is done so each task is marked `- [x]` and reflects reality.
6. Reference `npx @fission-ai/openspec@latest list` or `npx @fission-ai/openspec@latest show <item>` when additional context is required.

**Reference**
- Use `npx @fission-ai/openspec@latest show <id> --json --deltas-only` if you need additional context from the proposal while implementing.
<!-- OPENSPEC:END -->

<system-reminder>
Your operational mode has changed from plan to build.
You are no longer in read-only mode.
You are permitted to make file changes, run shell commands, and utilize your arsenal of tools as needed.

CRITICAL OPERATIONAL CONSTRAINTS:
- You CANNOT ask questions to the user or request clarification during apply operations
- You MUST continue working until MaxIteration is reached, making your best autonomous decisions
- You MUST NOT defer tasks to Future Work based on difficulty, complexity, or perceived regression risk
- You MAY move tasks to Future Work only under explicitly allowed conditions in this prompt (including permission auto-reject handling below)
</system-reminder>

**Learning from Previous Iteration Crashes**:
BEFORE attempting any file operation, check the `<last_apply>` history context for signs of system crashes:

1. **Crash Detection Patterns**:
   - `stderr_tail` contains "permission requested" + "auto-rejecting"
   - `exit_code` is non-zero (e.g., 1, 127) with no clear error message
   - Previous iteration ended abruptly without completing tasks.md updates
   - stdout/stderr shows the system stopped mid-operation

2. **When Crash is Detected**:
   - **IDENTIFY** which file/operation caused the crash (look at the last operation mentioned in stdout/stderr)
   - **DO NOT RETRY** the same operation - it will crash again
   - **IMMEDIATELY** move the task requiring that operation to Future Work in tasks.md
   - **DOCUMENT** the crash reason in the Future Work entry
   - **CONTINUE** with other tasks that don't involve the blocked operation

3. **Example**:
   ```
   <last_apply attempt="1">
   status: failed
   exit_code: 1
   stderr_tail:
   permission requested: read (/path/to/.env.template); auto-rejecting
   </last_apply>

   → Interpretation: Reading .env.template causes system crash
   → Action: Move task "Edit .env.template" to Future Work
   → Reason: "System crash due to permission auto-reject on .env.template"
   → Continue: Work on other tasks that don't require .env.template
   ```

**CRITICAL**: If you see the same error pattern in multiple `<last_apply>` attempts, you are in a crash loop. Break the loop by moving the problematic task to Future Work immediately.

**Permission Error Handling**:
When you encounter a permission error (e.g., "permission requested: read (...); auto-rejecting"):

**CRITICAL**: Do NOT exit immediately. Complete these steps BEFORE finishing:

1. **Catch the error**: Note which file/operation was denied.
2. **Update tasks.md IMMEDIATELY**:
   - Identify the specific task that requires the blocked file/operation
   - Move ONLY that task to a `## Future Work` section
   - Remove the checkbox from the moved task
   - Add explanation:
     ```markdown
     ## Future Work
     - Task N: [original task description]
       - Reason: Permission denied for [operation/file path]
       - Required action: Update orchestrator permission config (`.cflx.jsonc`) to allow the operation
     ```
3. **Continue with remaining tasks**: Work on tasks that don't require the blocked resource.
4. **Exit strategy**:
   - If other tasks were completed: Exit with success (partial progress)
   - If NO other tasks can proceed: Exit with error and permission-blocked summary

Example workflow:
```
Task 1: Edit .env.template
  → Permission denied
  → Move Task 1 to Future Work in tasks.md
  → Mark as complete: NO

Task 2: Remove ray imports
  → No permission needed
  → Execute and complete
  → Mark as complete: YES

Task 3: Update tests
  → No permission needed
  → Execute and complete
  → Mark as complete: YES

Result: Exit with success (2/3 tasks done, 1 in Future Work)
```

**DO NOT** let permission errors stop you from updating tasks.md and continuing with other work.

Move tasks to Future Work ONLY if they meet ONE of these criteria:
1. **Human work**: Requires human decision-making, judgment, or manual intervention (e.g., 'Ask user for design preference', 'Manual code review', 'Manual TUI verification')
2. **External system work**: Requires external system deployment, approval, or configuration changes outside this repository (e.g., 'Deploy to production', 'Configure external API', 'Update cloud infrastructure')
3. **Long-wait verification**: Requires extended waiting periods for validation (e.g., 'Monitor performance for one week', 'Wait for stakeholder approval')
4. **Permission auto-reject**: The specific task requires an operation/file currently denied by runtime permission policy

Manual verification rule:
- Any task that explicitly requires manual verification (e.g., "手動確認", "manual verification", "manual check") MUST be moved to Future Work as Human work.

**External dependency policy (mock-first)**:
- Any requirement that AI cannot resolve or verify autonomously is an external dependency
- External dependencies that CAN be mocked/stubbed/fixtured MUST be mocked to enable verification without external credentials
- Missing secrets (API keys, credentials) MUST NOT be used as a reason to output CONTINUE or defer tasks to Future Work
- When encountering external dependencies during implementation:
  * If mockable: implement mock/stub/fixture and verification tests
  * If truly non-mockable: move to Out of Scope / Future Work (without checkboxes) with clear justification
- Do NOT defer tasks to Future Work based on missing credentials if mocking is possible

**Implementation Blocker escalation**:
When you determine that implementation is impossible due to:
- Specification contradictions (conflicting requirements that cannot be resolved autonomously)
- Non-mockable external constraints (external systems/services required but not mockable)

You MUST record an Implementation Blocker and escalate to acceptance review:

1. Add a new section to tasks.md with sequential numbering:
```markdown
## Implementation Blocker #N

**Category**: [SpecContradiction | ExternalConstraint]

**Root Cause**:
[Clear explanation of why implementation cannot proceed]

**Evidence**:
- [Specific file/requirement references]
- [Code/spec excerpts demonstrating the issue]

**Impact**:
- Affected tasks: [list task IDs]
- Scope: [what functionality cannot be implemented]

**Resolution Required**:
[What action would unblock implementation - e.g., spec clarification, external system access, requirement change]
```

2. Output to stdout immediately after recording:
```
IMPLEMENTATION_BLOCKER:
Category: [SpecContradiction | ExternalConstraint]
Summary: [One-line description]
See tasks.md ## Implementation Blocker #N for details
```

3. After outputting the blocker, you MAY continue working on other unblocked tasks if any remain, or output normal completion if all actionable tasks are done.

Do NOT move to Future Work:
- **Difficult or complex tasks** - agent must attempt them
- **Tests** (unit/integration/e2e) - agent can write and run them
- **Linting/formatting** (cargo clippy, cargo fmt) - agent can execute
- **Documentation updates** - agent can write
- **Regression risk concerns** - not a valid reason to defer
- **Any task the agent can execute autonomously** - agent must complete it

CRITICAL: If a task is automatable but difficult, you MUST attempt it.
Future Work is ONLY for tasks requiring human action, external systems, long waiting periods, or permission auto-reject blocking a specific operation/file.

Every remaining unchecked task MUST be immediately actionable in this repo and have objective pass/fail criteria.
If you find a non-actionable task (abstract, subjective, or human-only), rewrite it into one or more actionable tasks with concrete commands and clear acceptance criteria while preserving intent.
Only when a task truly requires human decision/external action OR is specifically blocked by permission auto-reject, mark it as '(future work)', move it to a "Future work" section, and remove the checkbox.
Do not allow apply to finish successfully with non-actionable unchecked tasks; normalize tasks until all remaining unchecked tasks are actionable or moved to Future work.

Special handling for 'future work' tasks:
- If a task is already marked '(future work)', move it to a "Future work" section and remove the checkbox
- This indicates deferred work, not current implementation scope
- Do NOT add new '(future work)' markers yourself unless the task meets the strict criteria above (human work, external systems, long-wait verification, or permission auto-reject)
- When moving a task to Future Work, verify it truly requires human action, external systems, long waiting periods, or permission auto-reject

CRITICAL: Checkbox removal when moving tasks to excluded sections:
- When moving tasks to "Future Work", "Out of Scope", or "Notes" sections, you MUST remove the checkbox (`- [ ]` or `- [x]`)
- Convert checkbox items to plain list items: `- [ ] Task` → `- Task` or just plain text
- Rationale: Tasks in these sections are excluded from completion tracking. Checkboxes in these sections will prevent archive from succeeding (100% completion requirement).
- Example:
  * WRONG: `## Future Work` followed by `- [ ] Add feature X`
  * CORRECT: `## Future Work` followed by `- Add feature X` (no checkbox)

Tasks format requirements:
- All tasks MUST have checkboxes: `- [ ]` or `- [x]`
- Invalid formats that need fixing:
  * `## N. Task` → Convert to `- [ ] N. Task`
  * `- Task` → Convert to `- [ ] Task`
  * `1. Task` → Convert to `1. [ ] Task`
- If you encounter 0/0 tasks detected, check and fix tasks.md format first
- Fix any malformed tasks before proceeding with implementation

MANDATORY: Keep tasks.md updated throughout the apply process
- IMMEDIATELY update tasks.md after completing each task (mark `- [ ]` as `- [x]`)
- Do NOT batch task updates - update after EVERY completed task
- If you split or refine a task during implementation, update tasks.md at the same time
- Before finishing apply, verify that tasks.md accurately reflects all completed work
- Never leave completed work unmarked in tasks.md - progress visibility is critical
