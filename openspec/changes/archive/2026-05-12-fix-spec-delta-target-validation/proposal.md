---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/archive-promotion/spec.md
  - skills/cflx-proposal/SKILL.md
  - src/openspec_cmd.rs
---

# Fix spec delta target validation before archive

**Change Type**: implementation

## Premise / Context

- A recent archive failure in an external Conflux-run repository reached archive before surfacing missing canonical spec targets for `MODIFIED Requirements`.
- The native archive promotion path already rejects missing `MODIFIED` and `REMOVED` targets, but current strict proposal validation only checks delta markers and scenarios.
- The bundled `cflx-proposal` skill already tells authors to gather context and validate proposals, but it does not explicitly require canonical requirement-heading lookup before writing `MODIFIED` or `REMOVED` deltas.
- `openspec/CONSTITUTION.md` requires truthful completion based on repository-verifiable evidence, not narrative claims or hidden runtime state.

## Problem / Context

Conflux can currently accept or proceed with a proposal whose spec delta contains a `## MODIFIED Requirements` block targeting a requirement heading that does not exist in the canonical spec. The failure then appears later during archive promotion as `MODIFIED target not found in canonical spec`, after implementation work has already completed.

That is the wrong phase. Missing canonical spec targets are deterministic authoring errors and should be caught by proposal validation and by proposal authoring guidance before archive.

## Proposed Solution

Update the native OpenSpec validator and bundled proposal skill so missing `MODIFIED` and `REMOVED` target headings are detected before archive:

1. `cflx openspec validate <id> --strict` checks each spec delta against the matching canonical spec file.
2. Missing `MODIFIED` and `REMOVED` targets fail strict validation with diagnostics equivalent to archive promotion failures.
3. `cflx openspec validate <id> --archive-gate` also fails with the same target diagnostics.
4. The `cflx-proposal` skill instructs authors to inspect existing canonical requirement headings before choosing `MODIFIED` vs `ADDED`, and to run strict validation after authoring.
5. Tests cover valid modified targets, missing modified targets, missing removed targets, and missing canonical spec files where appropriate.

## Acceptance Criteria

- Strict validation fails before archive when a change delta modifies a requirement that is absent from the canonical spec.
- Strict validation fails before archive when a change delta removes a requirement that is absent from the canonical spec.
- Valid deltas with matching canonical targets continue to pass strict validation.
- `ADDED Requirements` for new requirements remain accepted without requiring a canonical target.
- Bundled `cflx-proposal` guidance prevents agents from blindly writing `MODIFIED Requirements` without checking canonical headings.
- Archive failure reporting remains compatible with the existing archive promotion contract and does not rely on external logs or durable workflow state.

## Explicit Completion Conditions

- `src/openspec_cmd.rs` includes canonical target validation in the strict proposal validation path, not only in archive promotion.
- Unit tests in `src/openspec_cmd.rs` or adjacent validator tests prove strict validation catches missing `MODIFIED` and `REMOVED` targets before archive.
- `skills/cflx-proposal/SKILL.md` includes explicit canonical heading lookup guidance for spec deltas.
- `cargo test openspec_cmd --lib` or a narrower equivalent validator test command passes.
- `cflx openspec validate fix-spec-delta-target-validation --strict --evidence warn` passes for this proposal.

## Out of Scope

- Changing archive promotion merge semantics.
- Changing OpenSpec requirement heading normalization rules beyond reusing existing canonical target matching behavior.
- Automatically rewriting incorrect external project proposals.
- Adding durable workflow-control state outside the workspace.
