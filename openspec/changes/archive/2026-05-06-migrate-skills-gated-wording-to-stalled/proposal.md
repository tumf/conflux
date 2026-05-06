---
change_type: implementation
priority: medium
dependencies: []
references:
  - skills/cflx-accept/SKILL.md
  - skills/cflx-accept-with-speca/SKILL.md
  - skills/cflx-workflow/SKILL.md
  - skills/cflx-workflow/references/cflx-accept.md
  - skills/cflx-archive/SKILL.md
  - skills/README.md
  - .opencode/commands/cflx-accept.md
  - openspec/specs/agent-prompts/spec.md
---

# Change: Migrate distributed skills from GATED wording to stalled implementation blocker terminology

**Change Type**: implementation

## Problem / Context

Conflux has already removed `gated` from user-facing lifecycle and display taxonomy: acceptance blockers pause changes as `stalled`, while `gated` remains only as an acceptance protocol compatibility term. The bundled skills installed by `cflx install-skills` still teach users and agents with `GATED`-centered headings, rubrics, and examples, which makes the distributed guidance look inconsistent with the current lifecycle model.

At the same time, the current runtime parser still expects `{"acceptance":"gated"}` / `ACCEPTANCE: GATED` for acceptance implementation-blocker handoff compatibility. Changing distributed skills to emit `{"acceptance":"stalled"}` before parser support would risk misrouting valid blockers on current or older runtimes.

## Proposed Solution

Update the distributed skill guidance so acceptance implementation blockers are described first as **stalled implementation blocker holds**. Keep `gated` only as the runtime-compatible protocol token that agents must emit until a later runtime/parser migration supports a stalled acceptance verdict.

The proposal updates acceptance-related bundled skills, their workflow references, the skills README, the OpenCode command mirror, embedded-skill drift tests, and canonical prompt specs so the installed user-facing guidance consistently says:

- operator-facing lifecycle/status is `stalled`
- a valid Implementation Blocker creates a stalled acceptance hold
- `{"acceptance":"gated"}` / `ACCEPTANCE: GATED` are compatibility handoff tokens, not preferred lifecycle vocabulary
- skills must not emit `{"acceptance":"stalled"}` yet unless a later runtime/parser proposal adds support

## Acceptance Criteria

- Distributed `skills/` acceptance guidance uses `stalled implementation blocker` or equivalent wording as the primary concept for valid acceptance blockers.
- `GATED` is not used as a user-facing lifecycle/status or rubric label in distributed skills.
- `{"acceptance":"gated"}` and `ACCEPTANCE: GATED` remain documented only as runtime-compatible protocol/fallback tokens for stalled acceptance holds.
- `cflx-accept-with-speca` continues to be drop-in compatible with `cflx-accept` and does not introduce a SPECA-specific verdict protocol.
- The OpenCode command template mirrors the same terminology even though the portable skill remains authoritative.
- Embedded skill contract tests and relevant documentation/spec tests are updated so the repository prevents regressions to `GATED`-centered distributed wording.

## Explicit Completion Conditions

The change is complete when repository evidence shows:

- the affected skill markdown and OpenCode command mirror no longer present `GATED` as a primary outcome name or FAIL-vs-GATED rubric for new guidance
- compatibility examples still show the exact runtime-safe tokens `{"acceptance":"gated"}` and `ACCEPTANCE: GATED`
- canonical `agent-prompts` spec delta requires distributed skills to frame acceptance blockers as stalled holds while preserving current parser compatibility
- Rust drift/contract tests cover the new wording boundary
- `cflx openspec validate migrate-skills-gated-wording-to-stalled --strict --evidence warn` and targeted repository tests pass

## Out of Scope

- Adding runtime parser support for `{"acceptance":"stalled"}`.
- Removing `gated` compatibility parsing from `src/acceptance.rs`.
- Renaming internal enum variants such as `AcceptanceResult::Gated` or `ParseResult::Gated`.
- Changing user-facing status derivation that already maps acceptance holds to `stalled`.
- Reworking non-acceptance skills except where they reference the distributed acceptance verdict contract.
