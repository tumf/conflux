---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - src/analyzer.rs
  - src/dependency_targets.rs
  - src/parallel_run_service.rs
  - src/parallel/queue_state.rs
  - openspec/specs/parallel-execution/spec.md
---

# Fix analysis archive root

**Change Type**: implementation

## Problem/Context

Recent Conflux logs after `.last-checked` show repeated `LLM analysis failed` entries for a single queued change whose frontmatter dependency was already archived:

- `fix-stale-merge-retry-worktree-status` depends on `fix-post-merge-deferred-false-warning`
- `fix-post-merge-deferred-false-warning` is present under `openspec/changes/archive/2026-05-10-fix-post-merge-deferred-false-warning`
- analyzer validation still classified it as a missing dependency target and fell back to metadata-only analysis repeatedly

The current analyzer collects archived/rejected dependency target evidence using `Path::new(".")` in `src/analyzer.rs`, so correctness depends on the process current working directory rather than the configured repository root. This violates the intended repository-visible dependency model and can misclassify archived dependencies as missing when Conflux is launched from a different cwd or when analysis runs in a context whose cwd is not the target repo root.

This proposal must obey `openspec/CONSTITUTION.md`: dependency routing remains derived from repository/workspace file and git state, not logs, UI state, or hidden durable state.

## Proposed Solution

Make analyzer dependency-target classification repository-root explicit:

- Store or pass the configured repository root into `ParallelizationAnalyzer` and use it for `collect_archived_change_ids` and `collect_rejected_change_ids`.
- Ensure all analyzer creation paths, including `ParallelRunService`, initialize the analyzer with the same target repository root used for OpenSpec listing and scheduler execution.
- Keep fail-closed behavior for truly missing and rejected dependencies.
- Add regression coverage proving archived dependencies are accepted when the process cwd differs from the target repository root.

## Acceptance Criteria

- Archived dependency references MUST be classified using the target repository root, not the process cwd.
- A queued change depending on an archived change under the target repo MUST NOT produce `Missing dependency reference` solely because Conflux was launched from a different cwd.
- Truly missing dependency references MUST continue to fail closed with dedicated missing-dependency diagnostics.
- Rejected dependency references MUST continue to fail closed with dedicated rejected-dependency diagnostics.
- The implementation MUST remain compatible with workspace-local workflow state rules in `openspec/CONSTITUTION.md`.

## Explicit Completion Conditions

- `src/analyzer.rs` no longer uses `Path::new(".")` for archived/rejected dependency evidence unless `.` is explicitly the configured target repo root.
- Analyzer constructors/call sites pass or store the target repo root used by the running Conflux instance.
- Unit or integration tests cover cwd-independent archived dependency classification, missing dependency fail-closed behavior, and rejected dependency fail-closed behavior.
- Existing fallback to metadata-only analysis in `src/parallel_run_service.rs` remains available for malformed LLM output, but valid archived dependencies no longer trigger that fallback as missing.

## Out of Scope

- Changing the semantics of proposal metadata dependencies.
- Treating rejected dependencies as satisfied.
- Reworking the broader LLM analysis prompt or queue scheduling model.
- Using log history, UI state, or any out-of-worktree cache as dependency evidence.
