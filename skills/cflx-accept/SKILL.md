---
name: cflx-accept
description: Portable Conflux acceptance operation skill. Defines the JSON-primary verdict interface and autonomous acceptance review guidance for any agent runtime. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Acceptance Review

Provides portable operation identity, verdict interface, and scoped acceptance guidance for Conflux orchestrator prompts.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

This skill identifies the current operation as acceptance review and defines the portable Conflux acceptance interface. It is agent-runtime independent and may be loaded by any supported agent runtime. Runtime-specific entrypoints are adapters that may mirror this contract, but they are not the authoritative interface for this skill.

If `openspec/CONSTITUTION.md` exists, read it before acceptance review and treat it as higher-priority project law than proposal/spec deltas when judging correctness.

## Operation Identity

- **Mode**: Acceptance review
- **Goal**: Verify implementation meets specifications with automated checks
- **Output**: Exactly ONE machine-readable verdict at the end

## Verdict Output Contract

**Primary (preferred)** — emit a strict JSON verdict object as the final
machine-readable payload, on its own line:

- PASS:     `{"acceptance":"pass"}`
- FAIL:     `{"acceptance":"fail","findings":["<evidence>"]}`
- CONTINUE: `{"acceptance":"continue"}`
- STALLED HOLD (compatibility token): `{"acceptance":"gated"}`

The JSON verdict is the canonical machine-readable contract. The Conflux runtime parser resolves it with priority over the legacy plain-text marker, including when the JSON verdict is wrapped inside a supported agent event payload and the runtime can unwrap the text. Do not rely on a specific agent runtime for this behavior.

**Fallback (backward-compatible)** — older runs still recognize the legacy
standalone plain-text markers on their own line:

- `ACCEPTANCE: PASS`
- `ACCEPTANCE: FAIL`
- `ACCEPTANCE: CONTINUE`
- `ACCEPTANCE: GATED` (legacy fallback for a stalled implementation blocker hold)
- Legacy fallback accepted during migration: `ACCEPTANCE: BLOCKED`

These markers are kept as a fallback so existing runs do not break. New
acceptance runs SHOULD emit the JSON verdict; when both appear, JSON wins.

**Transition guidance — emit BOTH during rollout** — until all running
Conflux orchestrator processes have been rebuilt with the JSON-aware
acceptance parser, the agent MUST emit BOTH payloads as the final two lines
of stdout (JSON verdict first, legacy marker second), each on its own line
with no markdown wrapping. Newer runtimes resolve the JSON verdict first
and finalize; older runtimes still finalize on the legacy marker. The
canonical contract remains JSON-primary.

Do not emit alternate schemas, extra machine-readable verdict objects, or provider-specific terminal markers.

## Scoped Guidance

### Declared Verification Phases

Structured `proposal.md` frontmatter verification declarations are authoritative over prose. For `pre-integration`, evaluate current-revision repository evidence and runnable local verification. For `post-integration`, evaluate repository-automation ownership, the tracked automation, trigger, evidence publication contract, rerun action, prerequisites, and fixture/local evidence without fetching an undeployed or external target.

Missing, placeholder, or incorrectly wired repository automation is a repository-fixable FAIL. A correctly wired post-integration declaration whose operational result is pending is not a FAIL. A non-mockable external prerequisite that makes declared automation unusable is a stalled hold; preserve the prerequisite owner and next rerun or unblock action. Never describe an unobserved post-integration operational outcome as successful.

### Verification Planning & Ownership

Acceptance owns proposal-quality judgment for behavior-changing work. When runtime or user-visible behavior is claimed, acceptance MUST determine whether tasks and repository evidence identify concrete implementation-facing work and integration points. Missing adequacy is an acceptance FAIL finding (not an archive blocker).

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
- Acceptance is read-only review. Do not edit `tasks.md` or the runtime-owned `## Current Acceptance Follow-up` section, and do not convert findings into checkbox tasks. Return repository findings and external blockers with concrete evidence and next actions; runtime classifies and persists them.
- Final OpenSpec validation, archive-gate validation, and archive readiness are not implementation tasks; if they need to be documented, require a non-checkbox `## Final Validation` or notes section.
- A valid `Implementation Blocker #<n>` with concrete evidence and unblock actions creates a stalled acceptance hold for operators and lifecycle/status displays.
- Recoverable infrastructure blockers are non-terminal stalled holds, not rejection evidence. Examples include Docker daemon/image pull failures, DNS/network timeouts, package registry outages, missing non-mockable credentials, port conflicts, and pending managed verification jobs.
- For the current runtime compatibility period, emit `{"acceptance":"gated"}` and the legacy fallback marker `ACCEPTANCE: GATED` only as protocol handoff tokens for that stalled hold.
- Legacy `blocked` acceptance verdict is input compatibility; `gated` is also compatibility/protocol terminology and MUST NOT be treated as operator-facing lifecycle taxonomy.
- Do not require or create terminal `REJECTED.md` evidence for recoverable infrastructure blockers unless independent evidence proves the change intent is invalid, obsolete, contradictory, or constitution-violating.
- Repository-fixable vs stalled-hold rubric:
  - `FAIL`: repository-only autonomous work (code/tests/spec/tasks/docs in this repo) can resolve the issue.
  - Stalled hold via compatibility token `{"acceptance":"gated"}`: repository-only work cannot resolve it in apply (human decision, repo-external prerequisite, unresolved external dependency, missing upstream constraint resolution, or recoverable infrastructure/credential/pending verification blocker).
- For behavior-changing work, missing/ambiguous verification planning is FAIL (not CONTINUE)

## Portable Interface Constraint

This operation skill owns a portable acceptance interface for Conflux agents. Runtime-specific entrypoints may mirror this interface, but this skill MUST NOT require an agent to inspect runtime-specific command directories or be invoked through a particular command mechanism in order to produce the correct verdict.

## Built-in Tools

```bash
# Show change details
cflx openspec show <id>

# Validate change
cflx openspec validate <id> --strict
```
