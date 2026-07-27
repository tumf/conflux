---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel_run_service.rs
  - src/orchestrator.rs
  - src/parallel/tests/executor.rs
  - openspec/specs/observability/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-05-13-classify-recoverable-analysis-fallback/
verifications:
  - id: recoverable-analysis-event-regression
    requirement: Successful metadata fallback remains warning-visible without emitting a terminal error event
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: targeted Rust test output proving warning-only event emission, deduplication, and metadata dependency preservation
    rerun: cargo test recoverable_analysis_fallback
    prerequisites: []
---

# Fix Recoverable Analysis Fallback Event Classification

**Change Type**: implementation

## Problem / Context

Conflux rejects malformed or incomplete LLM dependency-analysis output and safely continues with metadata-dependency-only analysis. Canonical specifications already classify this as degraded execution rather than a terminal workflow failure when fallback succeeds.

The tracing path follows that contract by logging a warning, but `ParallelRunService::emit_analysis_failure_diagnostic_once` also sends `ParallelEvent::Error`. The TUI and other event consumers therefore present a successful fallback as `Dependency analysis failed`, even while the scheduler continues safely. This contradicts the established observability contract and obscures actual terminal errors.

The observed production case involved an LLM response that omitted three of 22 queued change IDs. Validation correctly rejected the response, metadata fallback preserved the proposal dependency chain, and execution continued; only the emitted event classification was wrong.

## Proposed Solution

Change the recoverable fallback diagnostic to emit one deduplicated warning log event that explicitly says metadata dependency analysis is being used. Do not emit `ParallelEvent::Error` after fallback has been successfully constructed.

Preserve the original analysis error as warning context, keep metadata dependencies and scheduler fail-closed behavior unchanged, and retain terminal error events for paths where no safe fallback exists or another operation actually fails.

## Acceptance Criteria

- An invalid, incomplete, duplicate, or otherwise rejected LLM analysis followed by successful metadata fallback emits an operator-visible warning that names the fallback mode and original reason.
- The successful fallback path emits no `ParallelEvent::Error` and is not shown as a terminal dependency-analysis failure.
- Repeated equivalent fallback diagnostics remain deduplicated; a materially different error or queued/in-flight set remains independently visible.
- Metadata fallback includes every queued change exactly once and preserves declared proposal dependencies.
- Missing or rejected dependency targets remain actionable blockers and terminal errors elsewhere retain their existing event semantics.
- Workflow decisions remain derived from repository and runtime scheduler evidence, never from log or TUI classification.

## Explicit Completion Conditions

- `src/parallel_run_service.rs` separates recoverable fallback warning emission from terminal analysis error event emission.
- A targeted integration-style unit test drives malformed analysis through `analyze_order_with_sender`, reads the resulting event channel, and fails if any `ParallelEvent::Error` is emitted.
- Tests prove the warning message identifies metadata fallback, preserves the original parse/validation reason, and remains deduplicated.
- Existing metadata dependency preservation and dependency blocker tests pass.
- `cargo fmt --check`, targeted tests, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` pass.

## Out of Scope

- Repairing or retrying incomplete LLM analysis output.
- Changing analyzer completeness validation or prompt structure.
- Changing metadata dependency semantics, dispatch eligibility, or dependency blocker classification.
- Suppressing real terminal analysis, scheduler, merge, apply, acceptance, or archive failures.
