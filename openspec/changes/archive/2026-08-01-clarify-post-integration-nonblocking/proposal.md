---
change_type: implementation
priority: high
dependencies: []
references:
  - skills/cflx-accept/SKILL.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/changes/archive/2026-08-01-add-compressed-pairing-qr-import
verifications:
  - id: skill-post-integration-nonblocking
    requirement: cflx-accept skill clarifies that post-integration operational-observation verifications never stall
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: skills/cflx-accept/SKILL.md
    evidence: skill file diff and spec validation
    rerun: cflx openspec validate clarify-post-integration-nonblocking --strict
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Clarify post-integration operational-observation is never blocking

**Change Type**: implementation

## Problem / Context

The `cflx-accept` skill currently says:

> "A non-mockable external prerequisite that makes declared automation unusable is a stalled hold"

This statement is unconditionally applied to ALL verifications, including `post-integration` verifications with `completion_role: operational-observation`. As discovered in the `add-compressed-pairing-qr-import` change in `~/work2/latch`:

1. `physical-scan-observation` is declared `phase: post-integration`, `completion_role: operational-observation`, with prerequisite `compatible Corvus compressed-QR build`
2. The prerequisite doesn't exist yet (it's a future Corvus release)
3. The acceptance agent followed the skill rule literally: "non-mockable prerequisite → stalled hold"
4. The change was incorrectly stalled on category `infrastructure`

But `completion_role: operational-observation` means the verification is **explicitly non-blocking** by design. The `cflx-proposal` skill's own guidance says:

> "`completion_role: change-blocking` requires `phase: pre-integration` **and** `execution_class: repository-local`. Nothing else may block Conflux acceptance, archive, or merge."
> "Every `post-integration` declaration, and every non-local execution class, MUST be `completion_role: operational-observation`."

The acceptance skill needs to align with this: post-integration operational-observation verifications must never produce FAIL or stalled holds just because their prerequisites or execution are unavailable at acceptance time.

## Proposed Solution

1. **Update `skills/cflx-accept/SKILL.md`**:
   - Replace the unconditional "non-mockable external prerequisite → stalled hold" rule with a conditional rule scoped to `phase` and `completion_role`
   - Add explicit guidance: `post-integration` + `completion_role: operational-observation` → acknowledge as pending, do not stall
   - Keep the stalled hold eligible only for `pre-integration` + `completion_role: change-blocking` verifications with non-mockable prerequisites
   - Add an explicit rubric table: when to FAIL vs PASS vs stall based on verification metadata

2. **Update `openspec/specs/agent-prompts/spec.md`**:
   - MODIFY `Requirement: Acceptance MUST honor declared verification phases` to scope stalled-hold eligibility to `completion_role: change-blocking` only

## Acceptance Criteria

- Acceptance agent reading the updated skill does NOT return `gated`/stalled for post-integration operational-observation verifications with unavailable prerequisites.
- Acceptance agent acknowledges post-integration operational observations as pending without requiring them to pass.
- Pre-integration change-blocking verifications still produce stalled holds when non-mockable prerequisites are missing.
- The rubric is clear enough that an agent does not need to infer from narrative prose.

## Explicit Completion Conditions

- `skills/cflx-accept/SKILL.md` contains explicit `completion_role`-aware guidance in the `Declared Verification Phases` section.
- `openspec/specs/agent-prompts/spec.md` MODIFIED requirement scopes stalled-hold to `completion_role: change-blocking`.
- `cflx openspec validate clarify-post-integration-nonblocking --strict` passes.
- Manual acceptance review of a change with post-integration operational-observation + unavailable prerequisite yields PASS (or FAIL for unrelated reasons), never stalled.

## Out of Scope

- Changing the cflx-proposal skill (already correct).
- Changing Conflux runtime code (separate proposal `remove-acceptance-stall-persistence`).
- Changing the structured blocker contract fields.
