---
name: cflx-accept
description: Operation identity and scoped guidance for Conflux acceptance review. The fixed acceptance procedure remains defined by the .opencode/commands/cflx-accept.md command template. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Acceptance Review (Operation Identity)

Provides operation identity and scoped acceptance guidance for Conflux orchestrator prompts.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

This skill identifies the current operation as acceptance review and provides scoped guidance. The fixed acceptance procedure (checklist, verdict workflow, output format) remains defined by `.opencode/commands/cflx-accept.md` as the single source of truth.

This skill does NOT replace the command template. It supplements the orchestrator prompt with operation identity so the agent knows which operation mode it is in.

## Operation Identity

- **Mode**: Acceptance review
- **Goal**: Verify implementation meets specifications with automated checks
- **Output**: Exactly ONE legacy standalone verdict marker at the end

## Verdict Output Contract

For current Conflux compatibility, emit exactly ONE legacy standalone
plain-text verdict marker on its own line:

- `ACCEPTANCE: PASS`
- `ACCEPTANCE: FAIL`
- `ACCEPTANCE: CONTINUE`
- `ACCEPTANCE: BLOCKED`

Some currently running orchestrators still recognize only this legacy marker
contract. Do NOT rely on JSON-only verdict output from this worktree until the
runtime rollout is complete.

The full verdict contract (forbidden wrappings, findings format, retry
semantics) is owned by `.opencode/commands/cflx-accept.md`; this skill MUST
NOT redefine it.

## Scoped Guidance

### Verification Planning & Ownership

Acceptance MUST enforce the verification ownership planned by proposal/task guidance:

- Determine planned verification type per requirement/task (`unit`, `integration`, `e2e`, `manual`, `benchmark`, `not-testable`).
- Distinguish missing coverage from intentional coverage:
  - `manual` is intentional when explicit ownership/procedure is documented.
  - `benchmark` is intentional when expected performance evidence ownership is documented.
  - `not-testable` is intentional only when rationale and operational ownership are explicit.
- Do not fail solely because unit/integration tests are absent when planned verification is `manual`, `benchmark`, or `not-testable` and ownership is explicit.
- Fail when planned verification is missing or ambiguous for behavior-changing work; findings must call out planning/enforcement misalignment.
- For planned `unit`, integration-style evidence is a mismatch, not valid unit completion.

### Unit vs Integration Mismatch Handling

When a task claims unit verification ownership but evidence is integration-style:

1. Report a checklist truthfulness finding with concrete boundary evidence.
2. Require follow-up to either:
   - extract pure decision logic and add true unit tests, or
   - reclassify ownership/evidence as integration/e2e/manual/benchmark and update checklist claims.
3. Do not count integration-style evidence as unit-test completion.

### Spec-Only Change Detection

Before running checks, read `proposal.md` and detect the `Change Type` field:
- If `Change Type: spec-only` -> apply Spec-Only Acceptance path
- Otherwise -> apply the standard implementation acceptance path

### Accept Rules

- Each finding must include concrete evidence (file path, function, line)
- Each finding must be actionable by AI agent
- Missing secrets MUST NOT cause CONTINUE if mocking is possible
- Dirty working tree is always FAIL
- `ACCEPTANCE: BLOCKED` is allowed only when a valid `Implementation Blocker #<n>` exists with concrete evidence and unblock actions
- For behavior-changing work, missing/ambiguous verification planning is FAIL (not CONTINUE)

## Single-Source Constraint

The fixed acceptance procedure MUST remain defined by `.opencode/commands/cflx-accept.md`. This skill MUST NOT duplicate or override that command template's checklist, verdict workflow, or output format rules.

## Built-in Tools

```bash
# Show change details
cflx openspec show <id>

# Validate change
cflx openspec validate <id> --strict
```
