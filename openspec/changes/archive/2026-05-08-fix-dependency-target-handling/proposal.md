---
change_type: implementation
priority: high
dependencies: []
references:
  - src/analyzer.rs
  - src/parallel_run_service.rs
  - src/parallel/queue_state.rs
  - src/openspec.rs
  - src/openspec_cmd.rs
  - src/execution/state.rs
  - openspec/specs/parallel-execution/spec.md
---

# Fix Dependency Target Handling

**Change Type**: implementation

## Problem / Context

Parallel scheduling currently has inconsistent dependency semantics across proposal metadata parsing, analyzer validation, fallback analysis, and scheduler dispatch. Proposal frontmatter/body dependencies can be dropped by analyzer fast paths or fallback paths, allowing a dependent change to start while its prerequisite is still applying. Conversely, archived dependency references can be treated as invalid analyzer dependencies even though validation already classifies them as archived warnings rather than missing errors.

This violates the expected dependency contract for Conflux: active or in-flight dependencies are hard gates, archived dependencies are already satisfied references, and missing dependencies are invalid/fail-closed.

## Proposed Solution

Normalize dependency target handling across analyzer and scheduler layers:

- Treat parsed proposal metadata dependencies as authoritative hard dependencies that are unioned into analysis results even when LLM analysis is skipped, fails, or omits them.
- Classify dependency targets consistently as queued, in-flight, archived, or missing across validation, analyzer parsing, and dispatch-time checks.
- Treat archived dependencies as satisfied references that do not block dispatch and do not trigger generic parse failures or terminal rejection.
- Treat queued and in-flight dependencies as unresolved until their archive state is merged to the base branch.
- Treat missing dependencies as invalid/fail-closed so dependent changes are not dispatched based on incomplete dependency information.

## Acceptance Criteria

- A single queued change with metadata dependencies preserves those dependencies in the analysis result.
- LLM analysis fallback preserves metadata dependencies instead of returning dependency-free analysis.
- LLM analysis output that omits metadata dependencies is corrected by unioning authoritative proposal metadata dependencies.
- A queued change depending on an in-flight/applying change is not dispatched until the dependency is resolved on the base branch.
- A queued change depending on an archived change is not rejected, not reported as generic invalid JSON/parse failure, and is eligible to dispatch when all non-archived dependencies are resolved.
- A queued change depending on a missing change is not dispatched and receives a dedicated missing-dependency diagnostic.
- Analyzer, validator, and scheduler use the same target classification semantics for queued, in-flight, archived, and missing dependency targets.

## Explicit Completion Conditions

Complete only when source code paths in `src/analyzer.rs`, `src/parallel_run_service.rs`, and `src/parallel/queue_state.rs` preserve metadata dependencies and use shared or equivalent dependency target classification semantics, with regression tests proving in-flight dependencies block dispatch, archived dependencies are satisfied, missing dependencies fail closed, and metadata dependencies survive single-change and fallback analysis paths.

## Out of Scope

- Changing the OpenSpec proposal metadata schema.
- Changing archive directory naming conventions.
- Changing the constitutional rule that workflow state must be derivable from workspace/base git state.
- Implementing product-specific dependency behavior for downstream projects such as `avacuscc-dbot`.
