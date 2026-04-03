---
change_type: implementation
priority: high
dependencies: []
references:
  - src/execution/archive.rs
  - src/orchestration/archive.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/agent-prompts/spec.md
  - .pre-commit-config.yaml
---

# Change: Add acceptance archive-readiness gate

**Change Type**: implementation

## Problem / Context

The current workflow allows apply iterations to create WIP commits with `--no-verify`, while archive finalization requires a real commit that runs repository hooks. This can hide lint or hook failures until the archive phase.

A recent failure demonstrated the gap: a workspace successfully progressed through apply and acceptance, then archive failed because the final `Archive: {change_id}` commit was rejected by pre-commit `clippy` checks. At that point the change had already been moved into archive paths inside the workspace, leaving the run to fail late with `Archive commit verification failed`.

The repository already defines archive commit fallback behavior and already requires acceptance to verify clean working trees and truthful unit/integration classification. What is missing is an explicit acceptance responsibility to verify that the workspace is ready for a real final archive commit under repository quality gates.

## Proposed Solution

Introduce an archive-readiness gate during acceptance.

This change will:
- require acceptance to verify that the current workspace can satisfy the repository's final commit quality gate before archive begins;
- surface hook / lint / formatting / test blockers during acceptance rather than during archive finalization;
- distinguish archive-readiness failures from ordinary acceptance findings so the operator can see that the workspace is not yet final-commit-ready;
- keep archive responsible for executing the final move/commit, while moving the primary prevention responsibility to acceptance.

## Acceptance Criteria

- Acceptance explicitly checks archive-readiness before a change is allowed to proceed to archive.
- If repository quality gates that would block the final archive commit fail, acceptance returns FAIL (or an equivalent non-pass verdict) before archive starts.
- Acceptance output clearly identifies archive-readiness blockers such as pre-commit hook failures, `clippy` failures, formatting failures, or equivalent final-commit blockers.
- Archive continues to execute the final archive commit, but late failures caused by already-detectable hook/lint issues are prevented by the earlier gate.

## Out of Scope

- Replacing archive-side verification.
- Removing pre-commit hooks from final archive commit creation.
- Defining a universal readiness command for every repository outside the current configurable/project-standard commands.

## Impact

- Affected specs: `parallel-execution`, `agent-prompts`
- Affected code: acceptance prompt construction, acceptance orchestration flow, archive diagnostics
