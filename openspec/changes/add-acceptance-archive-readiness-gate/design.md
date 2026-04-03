## Context

Conflux currently separates apply, acceptance, and archive. Apply snapshots may intentionally bypass hooks via `--no-verify` so progress tracking is not blocked, while archive finalization must create a real commit that satisfies repository hooks. This creates a gap where latent clippy/hook failures can remain hidden until archive.

The project already has canonical requirements around acceptance loops, clean working trees, truthful unit/integration classification, and direct archive commit fallback. The missing piece is an explicit archive-readiness responsibility in acceptance.

## Goals / Non-Goals

- Goals:
  - Detect final-commit blockers during acceptance instead of archive.
  - Keep acceptance truthful about whether a workspace is actually ready to be archived.
  - Preserve archive's role as executor of the final archive commit.
- Non-Goals:
  - Eliminating archive-side verification.
  - Replacing repository hooks.
  - Generalizing a fully pluggable readiness framework in this change.

## Decisions

- Decision: Put the primary prevention responsibility in acceptance.
  - Why: acceptance is the last review gate before archive and is the natural place to stop late-quality failures from reaching archive.
  - Alternatives considered: improve archive diagnostics only; rejected because that still fails too late.

- Decision: Reuse repository-standard quality gates where possible.
  - Why: a separate acceptance-only readiness policy would drift from what the final commit actually enforces.
  - Alternatives considered: create a lighter synthetic readiness heuristic; rejected because it can miss real hook failures.

- Decision: Keep archive responsible for final execution and final verification.
  - Why: archive still has to move files and confirm the final commit is complete.
  - Alternatives considered: move archive commit creation into acceptance; rejected because it blurs phase responsibilities.

## Risks / Trade-offs

- Acceptance may become slower if it runs heavier readiness checks.
  - Mitigation: scope checks to repository-standard final-commit gates and avoid redundant recomputation where possible.

- Some repositories may have expensive hooks.
  - Mitigation: reuse documented commands and keep diagnostics explicit so operators know why acceptance is slower.

- There may still be rare archive-only failures after readiness passes.
  - Mitigation: retain archive-side verification and improve diagnostics for the remaining true archive failures.

## Migration Plan

1. Add canonical spec requirements for archive-readiness in acceptance.
2. Update acceptance prompt/integration flow to run and interpret readiness checks.
3. Update failure reporting to distinguish readiness failures from archive failures.
4. Add regression tests and run full verification.

## Open Questions

- Whether readiness should eventually be exposed as a first-class configurable command, or remain derived from repository-standard gates.
- Whether acceptance should report a dedicated verdict label for readiness failures or reuse FAIL with structured findings.
