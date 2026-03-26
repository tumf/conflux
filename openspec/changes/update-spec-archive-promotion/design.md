## Context

Two separate skill-local `cflx.py` implementations currently perform spec promotion during archive. Both use the same append-only merge logic, so they silently ignore `MODIFIED` and `REMOVED` deltas even though proposal authoring already permits those delta types.

## Goals / Non-Goals

- Goals:
  - Share one promotion implementation across `cflx-workflow` and `cflx-proposal`.
  - Make archive promotion deterministic for `ADDED`, `MODIFIED`, and `REMOVED` requirements.
  - Detect and fail silent no-op promotion before archive claims success.
  - Define archive guidance in terms of canonical diff evidence.
- Non-Goals:
  - Replacing the upstream OpenSpec CLI.
  - Introducing a new spec delta syntax.

## Decisions

- Decision: parse canonical and delta specs into requirement blocks keyed by normalized `### Requirement:` headings.
  - Rationale: the repo already treats full requirement blocks as the unit of modification, and OpenSpec guidance requires copying the entire requirement block for `MODIFIED` deltas.

- Decision: expose one shared promotion API for both skill-local scripts.
  - Rationale: behavior drift between `cflx-workflow` and `cflx-proposal` would recreate the same false-success gap.

- Decision: add an archive-check simulation step that computes touched canonical specs and fails when promotion would not change them.
  - Rationale: syntactic validation alone cannot detect the specific failure mode uncovered in the session analysis.

- Decision: require archive guidance to reference canonical spec diffs, not helper output strings.
  - Rationale: helper output can be stale or misleading if promotion logic regresses again.

## Risks / Trade-offs

- Requirement-name matching relies on stable headings. This is acceptable because OpenSpec authoring already requires exact requirement-name matches for `MODIFIED` deltas.
- Adding archive-check logic increases validator scope, but the benefit is preventing silent spec corruption or false success in spec-only workflows.

## Migration Plan

1. Land the shared helper and switch both scripts to it.
2. Add archive-check simulation and failure conditions.
3. Update workflow guidance to describe the new verification expectations.
4. Add regression fixtures before removing any legacy merge code.

## Open Questions

- Whether the archive-check path should be enabled automatically during archive in addition to being exposed explicitly via validation.
