---
name: cflx-archive
description: Archive deployed OpenSpec changes and update canonical specs. Provides archive-specific guidance for Conflux orchestration. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Archive Executor

Archive deployed OpenSpec changes and update canonical specifications.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

After a change has been accepted, this skill handles archiving: moving the change to `changes/archive/`, promoting spec deltas to canonical specs, and verifying the result.

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
   ```
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
