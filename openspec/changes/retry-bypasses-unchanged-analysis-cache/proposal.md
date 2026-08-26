---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/orchestration/run_control.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/unchanged_analysis_input.rs
verifications:
  - id: explicit-retry-dispatch-regression
    requirement: An accepted retry_change against stalled queued work bypasses unchanged-input suppression once and reaches dispatch selection
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/unchanged_analysis_input.rs
    evidence: Focused Rust regression test output showing a second analysis attempt and dispatch after retry_change with an unchanged signature
    rerun: cargo test --lib retry_change_bypasses_unchanged_analysis_input_and_dispatches
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Make explicit retry bypass unchanged analysis suppression

**Change Type**: implementation

## Problem / Context

In Conflux v0.6.295, `retry_change` accepts a stalled change, moves it to reducer-visible `queued`, and wakes an idle scheduler. The scheduler then logs `Queue notification received while scheduler idle` followed by `No analysis started: reason=unchanged_analysis_input`; the queued work remains undispatched. Toggling queue intent off and on creates a different edge and allows the change to advance to `preparing`.

This violates the existing scheduler contract: an accepted explicit retry is a one-shot state-transition edge and must not reach the ordinary unchanged-input gate. The current source documents that contract, but the production `retry_change` path does not preserve sufficient edge identity through scheduler wake, reconciliation, and analysis gating.

## Proposed Solution

Preserve accepted explicit-retry identity from the shared run-control path through the live scheduler's next eligible evaluation. That evaluation must bypass debounce and `unchanged_analysis_input` suppression exactly once for the retried target, then return to ordinary suppression policy. Keep existing dependency, capacity, eligibility, and terminal-error guards intact.

Use one authoritative explicit-retry edge rather than relying on generic queue notification or on a changed queue signature. Do not clear or consume the edge before the scheduler performs the analysis/dispatch evaluation it authorizes.

## Acceptance Criteria

1. `retry_change` accepted for a stalled, retry-eligible change with reducer-visible queued work causes one immediate dependency-analysis attempt even when its analysis-input signature matches the last completed input.
2. If analysis selects the retried change and normal dependency/capacity guards allow it, the change advances to dispatch (`preparing`/Apply) without queue-intent toggling or another operator command.
3. The explicit-retry edge is consumed only after one eligible scheduler evaluation; subsequent unchanged timer wakes remain suppressed.
4. A retry edge for one change does not release another failed change or bypass dependency, capacity, eligibility, or retry-budget constraints.
5. Generic wake notifications without a consumed explicit-retry edge remain subject to ordinary debounce and unchanged-input suppression.

## Explicit Completion Conditions

- The production `retry_change` path carries target-specific explicit-retry identity into the scheduler evaluation that reaches the unchanged-input gate.
- A deterministic paused-time regression test starts from a completed matching signature, accepts `retry_change`, observes a second analyzer invocation, and proves dispatch begins without queue-intent mutation.
- The same test proves the one-shot edge does not replay on later timer wakes.
- Existing explicit-edge and unchanged-input suppression tests remain green.
- `cargo test --lib retry_change_bypasses_unchanged_analysis_input_and_dispatches` passes.

## Out of Scope

- Changing the analysis-input signature contents.
- Disabling unchanged-input suppression for ordinary timer or generic wake paths.
- Altering retry eligibility, dependency ordering, capacity limits, or repair-budget policy.
- Treating queue-intent toggling as the retry mechanism.
- Durable retry-edge state across process restart; the edge remains process-local ephemeral control state.

## Premise / Context

- Requested artifact: implementation proposal plus regression coverage.
- The reducer transition and command acknowledgement already succeed; the missing outcome is scheduler dispatch.
- `src/parallel/orchestration.rs` and `src/parallel/queue_state.rs` define explicit edges as one-shot bypasses of unchanged-input suppression.
- The fix must preserve the constitution's workspace-local durable routing and use only ephemeral process-local edge state.
