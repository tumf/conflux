---
name: cflx-workflow
description: Legacy compatibility router for Conflux workflow operations. Routes apply/rejecting/cleanup-review/accept/archive to self-contained operation guidance. New orchestrator prompts should use dedicated operation-specific skills (cflx-apply, cflx-accept, etc.) instead. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Workflow Compatibility Router

Self-contained legacy compatibility router for Conflux workflow operations. New orchestrator prompts use dedicated operation-specific skills (`cflx-analyze`, `cflx-apply`, `cflx-rejecting`, `cflx-cleanup-review`, `cflx-accept`, `cflx-archive`, `cflx-resolve`). This router ensures older prompts using `load skills: cflx-workflow` continue to work without additional skill loads or cross-skill references.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Constitutional Priority

- If `openspec/CONSTITUTION.md` exists, read it before routing or executing any operation and treat it as higher-priority project law than proposal/spec deltas.
- Do not route, approve, or implement changes that violate `openspec/CONSTITUTION.md` unless that constitution is explicitly changed first.

## Operation Selection

Parse the invocation to determine the operation:

- If change ID with "apply" or "implement" context -> Execute **Apply**
- If "rejecting" or "rejection review" context -> Execute **Rejecting Review**
- If "cleanup-review" context -> Execute **Cleanup Review**
- If "accept" or "acceptance" context -> Execute **Accept**
- If "archive" context -> Execute **Archive**

---

## Apply (Implementation)

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

4. **Update Progress Continuously** - Update `tasks.md` after each task; never batch updates

5. **Verify Completion** - Ensure all tasks are `[x]` or in Future Work; run final validation; confirm integration points

### Truthful Completion Rules

Before changing any task to `[x]`, verify:
1. The repository contains the required implementation artifact for that task.
2. The artifact is reachable from the intended flow when the task claims runtime integration.
3. The relevant verification command has been run successfully, or concrete blocker evidence has been recorded.
4. The task description still matches reality.
5. Unit-test completion is valid only when tests are genuinely unit-scoped (mocks/fakes/in-memory doubles only).
6. The evidence type is consistent with the planned verification type; mismatches MUST be recorded as follow-up work.

Never mark a task complete based only on `openspec/` edits, `tasks.md` normalization, archived/merged proposals, or stub placeholders.

### Task Management

**Move to Future Work ONLY if**: requires human judgment, external system access, long-wait verification (>1 day), or already marked '(future work)'.

**Do NOT move to Future Work**: difficult tasks, tests, linting, documentation, any automatable task.

**Checkbox Rules**: Active sections must have checkboxes. Excluded sections (Future Work, Out of Scope, Notes) must NOT have checkboxes.

### Implementation Blocker Escalation

If implementation is impossible, add `## Implementation Blocker #<n>` to `tasks.md` with category, summary, evidence, impact, unblock_actions, owner, and decision_due. Create `REJECTED.md` as an apply-generated rejection proposal artifact. Output `IMPLEMENTATION_BLOCKER:` marker.

### Apply Completion Criteria

- All tasks marked `[x]` or moved to Future Work (without checkboxes)
- Code compiles/builds, tests pass, lint passes
- Integration points verified
- Non-OpenSpec evidence exists for implementation tasks

**For detailed guidance**, read [references/cflx-apply.md](references/cflx-apply.md).

---

## Rejecting Review

**Purpose**: Review apply-generated rejection proposals in `REJECTED.md` before any terminal reject decision.

### Required Checks

1. Confirm `REJECTED.md` exists and contains a concrete reason.
2. Confirm blocker evidence in `tasks.md` (`## Implementation Blocker #N`) is specific and actionable.
3. Decide one outcome only:
   - `CONFIRM`: terminal rejection evidence proves invalid/obsolete/contradictory/constitution-violating change intent.
   - `RESUME`: reject proposal is dismissed; change returns to apply.
   - `BLOCK`: rejection proposal describes a real non-terminal infrastructure/pending-verification blocker; change becomes stalled.
4. Output exactly one final marker line: `REJECTION_REVIEW: CONFIRM`, `REJECTION_REVIEW: RESUME`, or `REJECTION_REVIEW: BLOCK`

### Outcome Rules

- On CONFIRM, runtime finalizes rejection flow and may write base-branch `REJECTED.md`.
- On RESUME, runtime removes worktree-local `REJECTED.md` and appends recovery task.
- On BLOCK, runtime removes worktree-local `REJECTED.md`, preserves the worktree, records a stalled hold, and MUST NOT write base-branch `REJECTED.md`.
- MUST NOT output `ACCEPTANCE: GATED`.

---

## Cleanup Review

**Purpose**: Ensure a task-complete but dirty managed worktree is made handoff-ready before acceptance starts.

### Required Behavior

1. Run inside the managed worktree for the given change.
2. Review dirty files and clean only post-apply handoff artifacts.
3. **NEVER** use blind staging such as `git add -A` or `git add .`.
4. Stage/commit only intentional cleanup changes required for clean handoff.
5. Verify worktree cleanliness before finishing.
6. Output exactly one success marker line on success: `CLEANUP_REVIEW: CLEAN`

### Output Rules

- Success output MUST contain exactly one standalone marker line: `CLEANUP_REVIEW: CLEAN`
- Do not emit alternate verdict markers or wrap the marker in code fences.
- If cleanup cannot be completed, fail loudly instead of inventing a different marker.

---

## Accept (Acceptance Review)

**Purpose**: Verify implementation meets specifications with automated checks.

**CRITICAL**: Output exactly ONE verdict at the end.

### Spec-Only Change Detection

Read `proposal.md` and detect `Change Type`:
- If `Change Type: spec-only` -> apply Spec-Only Acceptance path
- Otherwise -> standard implementation acceptance path

### Required Checks

1. **Git Working Tree Clean** - `git status --porcelain` must output empty
2. **Task Completion** - All tasks `[x]` or in Future Work; no checkboxes in excluded sections
3. **Spec Matching** - Implementation matches specification
4. **Integration Check** *(skip for spec-only)* - Feature executed in real flow
5. **Dead Code Check** *(skip for spec-only)* - All code is invoked
6. **No Stubbed Runtime Check** *(skip for spec-only)* - No mock/stub/fake in production path
7. **Regression Check** - Existing features not broken
8. **Evidence Citation** - Cite file path + function/method
9. **Checklist Truthfulness** - FAIL if tasks claim completion without repo evidence

### Output Format

Output exactly ONE machine-readable verdict on its own line with NO markdown
formatting.

**Primary (preferred)** — strict JSON verdict object:

- PASS:     `{"acceptance":"pass"}`
- FAIL:     `{"acceptance":"fail","findings":["<evidence>"]}` (findings mirrors the FINDINGS section)
- CONTINUE: `{"acceptance":"continue"}`
- STALLED HOLD (compatibility token): `{"acceptance":"gated"}`

**Fallback (backward-compatible)** — legacy standalone plain-text markers:

- `ACCEPTANCE: PASS` - All checks pass
- `ACCEPTANCE: FAIL` - Checks fail (followed by FINDINGS and tasks.md update)
- `ACCEPTANCE: CONTINUE` - Verification incomplete
- `ACCEPTANCE: GATED` - Legacy fallback for a valid Implementation Blocker stalled hold
- Legacy fallback accepted during migration: `ACCEPTANCE: BLOCKED`

The JSON verdict is primary; legacy markers remain supported so existing runs
do not break. When both are present, the JSON verdict wins.

**Transition guidance — emit BOTH during rollout** — until all running
Conflux orchestrator processes have been rebuilt with the JSON-aware
acceptance parser, the agent MUST emit BOTH payloads as the final two lines
of stdout (JSON verdict first, legacy marker second), each on its own line
with no markdown wrapping. Newer runtimes resolve the JSON verdict and
finalize; older runtimes still finalize on the legacy marker. The canonical
contract remains JSON-primary.

Do NOT wrap the verdict in headings (`##`), blockquotes (`>`), bullets (`-`), bold (`**`), or code fences.

### Accept Rules

- Each finding must include concrete evidence (file path, function, line)
- Missing secrets MUST NOT cause CONTINUE if mocking is possible
- Dirty working tree is always FAIL
- Valid `Implementation Blocker #<n>` creates a stalled acceptance hold; emit `{"acceptance":"gated"}` only as the current compatibility token for that hold
- Recoverable infrastructure blockers are non-terminal stalled holds, not terminal rejection evidence. Representative classes include Docker daemon/image pull failures, DNS/network timeouts, package registry outages, missing non-mockable credentials, port conflicts, and pending managed verification jobs.

**For detailed guidance**, read [references/cflx-accept.md](references/cflx-accept.md).

---

## Archive

**Purpose**: Archive deployed change and update canonical specs.

### Execution Steps

1. **Validate Change Status**
   ```bash
   cflx openspec list
   cflx openspec show <id>
   ```
2. **Run Archive**
   ```bash
   cflx openspec archive <id> --yes
   ```
3. **Verify Results** - Confirm moved to `changes/archive/`, specs updated, `cflx openspec validate --strict` passes
4. **Review canonical spec diff** - Run `git diff openspec/specs/` and verify expected requirement changes

**For detailed guidance**, read [references/cflx-archive.md](references/cflx-archive.md).

---

## Built-in Tools

```bash
cflx openspec list                          # List changes
cflx openspec list --specs                  # List specs
cflx openspec show <id>                     # Show change details
cflx openspec show <id> --json              # Show JSON output
cflx openspec show <id> --json --deltas-only # Show deltas only
cflx openspec validate <id> --strict        # Validate change
cflx openspec validate --strict             # Validate all
cflx openspec archive <id> --yes            # Archive change
cflx openspec archive <id> --yes --skip-specs # Archive without spec updates
```

## Autonomous Decision Framework

1. **Existing patterns** - Follow patterns in the codebase
2. **Specification** - Refer to spec deltas and scenarios
3. **Simplicity** - Choose simpler implementation
4. **Documentation** - Document decision in code comments

**Never**: Ask user for clarification, stop and wait for input, leave tasks incomplete due to uncertainty.

## Summary

| Operation | Trigger | Output | Constraints |
|-----------|---------|--------|-------------|
| Apply | "apply \<id\>" | Completed tasks + code | No questions, update immediately |
| Rejecting | "rejecting \<id\>" | CONFIRM / RESUME | Review blocker evidence |
| Cleanup | "cleanup-review \<id\>" | CLEANUP_REVIEW: CLEAN | No blind staging |
| Accept | "accept" | PASS/FAIL/CONTINUE/STALLED HOLD (`gated` compatibility token) | Output once, cite evidence |
| Archive | "archive \<id\>" | Archived change | Validate before/after |

**REMEMBER**: This skill operates autonomously. Never ask questions. Make decisions based on available context.
