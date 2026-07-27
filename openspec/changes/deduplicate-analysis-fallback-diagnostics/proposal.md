---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/parallel_run_service.rs
  - openspec/specs/observability/spec.md
verifications:
  - id: fallback-diagnostic-dedup
    requirement: Equivalent fallback diagnostics are deduplicated consistently across runtime and tracing observability
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output proving one warning per equivalent diagnostic signature and distinct warnings for changed signatures
    rerun: cargo test recoverable_analysis_fallback
    prerequisites: []
---

# Deduplicate Analysis Fallback Diagnostics

**Change Type**: implementation

## Problem / Context

Successful dependency-analysis fallback emits two warning representations: a tracing `warn!` record and a deduplicated runtime warning event. The tracing warning is emitted before deduplication, so repeated equivalent fallback failures remain suppressed in the TUI event stream but accumulate in persistent and terminal-facing tracing logs.

The canonical observability requirement says repeated equivalent diagnostics are deduplicated but does not distinguish runtime events from tracing records. The implementation and contract must agree so operators receive bounded diagnostics consistently.

## Proposed Solution

Apply the existing analysis-failure signature deduplication decision to both tracing and runtime warning emission. An equivalent queued set, in-flight set, and normalized rejection reason produces one warning diagnostic per deduplication lifetime. A materially changed reason or queued/in-flight context remains independently visible.

Keep warning-level classification, original failure context, metadata fallback naming, and scheduler behavior unchanged.

## Acceptance Criteria

- Repeated equivalent successful fallback diagnostics produce one warning tracing record and one warning runtime event within the existing deduplication lifetime.
- A changed rejection reason or queued/in-flight context emits a new warning through both observability surfaces.
- Successful fallback emits no error-level tracing record and no terminal error event.
- Diagnostic suppression never changes fallback analysis, dispatch, or workflow-control decisions.

## Explicit Completion Conditions

- The deduplication gate encloses both tracing warning and runtime warning emission for the same signature.
- Tests capture repeated tracing records and runtime events and verify equivalent suppression plus distinct-signature visibility.
- Existing tests continue to prove full queued order, metadata dependency preservation, and absence of terminal error events.
- `cargo test recoverable_analysis_fallback`, `cargo fmt --check`, and repository lint pass.

## Out of Scope

- Global deduplication of unrelated warning types.
- Changing deduplication lifetime or persistence.
- Changing analyzer acceptance rules or fallback dependency semantics.
