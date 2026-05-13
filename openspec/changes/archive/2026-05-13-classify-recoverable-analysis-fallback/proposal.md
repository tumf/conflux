---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/parallel_run_service.rs
  - src/analyzer.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/observability/spec.md
---

# Classify recoverable analysis fallback logs

**Change Type**: implementation

## Problem / Context

Parallel dependency analysis can reject an LLM-produced dependency graph and immediately recover by falling back to metadata-only dependency analysis. That recovery path is expected behavior when the fallback preserves declared metadata dependencies and keeps dispatch fail-closed for missing or rejected targets.

Today the failed LLM attempt is logged at error severity even when Conflux successfully falls back and continues with the authoritative metadata dependency graph. Runtime log mining therefore reports a large actionable error group for a recoverable condition, making it harder to distinguish true workflow failures from expected degradation.

The log-mining helper itself is observability-only and must not influence scheduler decisions.

## Proposed Solution

Classify recoverable dependency-analysis fallback as a warning/degraded-path diagnostic rather than an error-level workflow failure. Preserve strict error reporting when no safe fallback is available, when fallback construction fails, or when dispatch remains blocked by missing/rejected dependencies.

Keep existing metadata-dependency-only fallback semantics unchanged: the fallback must continue to include proposal metadata dependencies and must not become dependency-free.

## Acceptance Criteria

- Recoverable LLM dependency-analysis failures that successfully fall back to metadata-only analysis are not emitted as error-level log records.
- Operators can still see that LLM analysis failed and that Conflux degraded to metadata-only analysis.
- Missing/rejected dependency blockers remain visible as actionable warnings or errors and still prevent unsafe dispatch.
- No log severity classification is used as workflow-control input.

## Explicit Completion Conditions

- `src/parallel_run_service.rs` distinguishes recoverable analysis fallback from terminal analysis failure in log severity.
- Regression coverage proves metadata fallback still preserves declared dependencies and emits a non-error degraded-path diagnostic.
- Existing dependency blocker tests continue to prove missing/rejected targets fail closed.

## Out of Scope

- Changing dependency analysis semantics or prompt behavior.
- Changing scheduler dispatch eligibility for missing, rejected, archived, active, queued, or in-flight dependency targets.
- Persisting log-mining results or using logs as workflow-control state.
