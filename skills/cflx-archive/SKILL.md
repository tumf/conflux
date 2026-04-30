---
name: cflx-archive
description: Archive deployed OpenSpec changes and update canonical specs. Provides archive-specific guidance for Conflux orchestration. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Archive Executor

Archive deployed OpenSpec changes and update canonical specifications.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

After a change has been accepted, this skill handles archiving: moving the change to `changes/archive/`, promoting spec deltas to canonical specs, and verifying the result.

If `openspec/CONSTITUTION.md` exists, read it before archive validation and treat it as higher-priority project law than proposal/spec deltas when judging archive readiness.

Proposal-quality judgment (for example, behavior-task adequacy) belongs to acceptance review. Archive responsibility is limited to real archive-readiness and commit-path blockers (archive execution, canonical spec promotion, repository state), and MUST NOT reintroduce wording heuristic blockers.

## Acceptance Handoff Contract

Archive only runs after acceptance reached PASS. The acceptance verdict is
resolved by the runtime from the acceptance agent's stdout using the
following contract:

- **Primary**: strict JSON verdict object
  (`{"acceptance":"pass|fail|continue|gated", "findings":[...]}`) on its
  own line, possibly wrapped inside an `opencode run --format json` event
  payload.
- **Fallback**: legacy standalone plain-text markers (`ACCEPTANCE: PASS`,
  `ACCEPTANCE: FAIL`, `ACCEPTANCE: CONTINUE`, `ACCEPTANCE: GATED`) remain
  supported so older runs continue to hand off, but JSON wins when both are
  present.
- **Legacy compatibility**: runtimes MAY still accept legacy `blocked` acceptance
  verdict input during migration, but canonical output remains `gated`.

Archive MUST NOT redefine or relax this contract. When the upstream
acceptance verdict is ambiguous (missing JSON, malformed legacy marker), the
runtime returns CONTINUE and archive does not start — investigate upstream
acceptance output rather than working around it here.

## Execution Steps

1. **Identify Change ID**
   - From orchestrator invocation
   - Or from context (must be unambiguous)

2. **Validate Change Status**
   ```bash
   cflx openspec list
   cflx openspec show <id>
   ```
   - Ensure change exists
   - Ensure not already archived
   - Ensure ready for archive

3. **Run Archive**
   ```bash
   cflx openspec archive <id> --yes
   ```
   - Use `--skip-specs` only for tooling-only changes

4. **Verify Results**
   - Confirm moved to `changes/archive/`
   - Confirm specs updated
   ```bash
   cflx openspec validate --strict
   cflx openspec validate <id> --strict --evidence warn
   ```
   - Use only native evidence enum values (`off`, `warn`, `error`) when evidence validation is requested.
   - **Review canonical spec diff** -- run `git diff openspec/specs/` and verify each touched `openspec/specs/**` file shows the expected requirement changes. Do not rely solely on `Specs updated: [...]` output.

## Archive Completion Criteria

- Change moved to `openspec/changes/archive/<id>/`
- Canonical specs updated (unless `--skip-specs`)
- Validation passes with `--strict`
- `git diff openspec/specs/` confirms expected requirement additions, replacements, or removals for each touched spec

**For detailed guidance**, read [references/cflx-archive.md](references/cflx-archive.md).

## Built-in Tools

```bash
# List changes
cflx openspec list

# List specs
cflx openspec list --specs

# Show change details
cflx openspec show <id>

# Validate change
cflx openspec validate <id> --strict

# Validate all
cflx openspec validate --strict

# Archive change
cflx openspec archive <id> --yes

# Archive without spec updates
cflx openspec archive <id> --yes --skip-specs
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
- Leave archive incomplete due to uncertainty
