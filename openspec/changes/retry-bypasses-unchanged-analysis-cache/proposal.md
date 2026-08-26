---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/orchestration/run_control.rs
  - src/orchestration/operator_command.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/unchanged_analysis_input.rs
  - src/parallel/tests/change_error_f5_retry.rs
verifications:
  - id: explicit-retry-edge-creation
    requirement: Every accepted retry route arms a target-specific explicit-retry scheduler edge, and a refused or no-op retry arms none
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/operator_command.rs
    evidence: Focused Rust test output showing an accepted acceptance-stall retry and an accepted terminal-error retry each arm exactly one edge for their own target, that the stall route arms no failed-classification or Apply-budget release, and that a refused or no-op retry arms nothing
    rerun: cargo test --lib accepted_retry_publishes_explicit_retry_edge
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: explicit-retry-dispatch-regression
    requirement: An accepted retry against stalled reducer-visible queued work bypasses unchanged-input suppression once through the production scheduler loop and reaches real dispatch
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/change_error_f5_retry.rs
    evidence: Focused Rust regression output showing the production loop step consuming the armed edge without a caller-supplied reanalysis reason, invoking the analyzer against an unchanged signature, starting a real Apply dispatch for the retried change, and suppressing the next unchanged timer wake
    rerun: cargo test --lib retry_change_bypasses_unchanged_analysis_input_and_dispatches
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Make explicit retry bypass unchanged analysis suppression

**Change Type**: implementation

## Problem / Context

In Conflux v0.6.295, `retry_change` accepts a **stalled** change, moves it to reducer-visible `queued`, and wakes an idle scheduler. The scheduler then logs `Queue notification received while scheduler idle` followed by `No analysis started: reason=unchanged_analysis_input`; the queued work remains undispatched. Toggling queue intent off and on creates a real queue-membership change and allows the change to advance to `preparing`.

The scheduler-side bypass machinery already exists and works when an edge is armed: `consume_explicit_retry_edges` drains the edge at Step 0 of the loop, `execute_with_order_based_reanalysis` turns a consumed edge into `ReanalysisReason::QueueNotification`, and `bypasses_unchanged_input_gate` treats that reason as a bypass. What is missing is upstream, and it is route-specific:

- `classify_retry_route` routes `error` to `RetryRoute::TerminalError` and `stalled` (and externally blocked) to `RetryRoute::AcceptanceStall`.
- `apply_retry_route` publishes an explicit-retry edge only for the `TerminalError` route; the `AcceptanceStall` route applies `ReducerCommand::AddToQueue` and deliberately publishes nothing, because publishing today also releases the scheduler-local failed classification and resets the retried target's Apply budget.
- The stall route therefore reaches a live scheduler as a bare wake. It adds no scheduler-visible queued candidate either, so `has_queued_additions()` is false, the reason is reduced to `Initial`, and the ordinary unchanged-input gate suppresses the evaluation — exactly the observed log pair.

So the defect is not that an armed edge is lost in transit on the reproduced path: for a stalled target no edge is ever armed. A fix framed only as "preserve the edge better" would leave the reproduced case broken. The secondary hazard is real but separate: the Step 0 drain converts the edge into a pass-local boolean, so a pass that ends before analysis (cancellation `continue`, an incomplete reducer view, an early break) discards that authority with no edge left to replay it.

## Proposed Solution

Give **every accepted retry route** target-specific explicit-retry scheduler-edge authority, and keep that authority alive until an eligible dependency-analysis evaluation actually consumes it.

- Arm a target-specific edge for the accepted acceptance-stall route as well as the terminal-error route. `retry_change`, `retry_errors`, Start/F5 retry, and the terminal-error alias of an add-to-queue request all go through the same arming point.
- Keep the *authority carried by* an edge route-scoped and fail-closed: releasing a scheduler-local failed classification, dropping the blocked fingerprint, and resetting the Apply budget stay exclusive to the terminal-error route. A stall-route edge grants analysis-bypass authority only.
- The authorized evaluation bypasses queue debounce and `unchanged_analysis_input` suppression exactly once for the retried target, then returns to ordinary suppression policy.
- Do not lose the authority in a pass that ends before the analysis evaluation it authorizes; a discarded pass leaves the edge available to the next eligible evaluation rather than consuming it.
- Keep existing dependency, capacity, eligibility, terminal-error, and retry-budget guards intact.

Use one authoritative explicit-retry edge rather than relying on generic queue notification or on a changed queue signature. Expressing a consumed edge as the existing bypass-carrying reanalysis reason is not a downgrade; reducing it to an ordinarily suppressible wake is.

## Acceptance Criteria

1. An accepted `retry_change` for a **stalled**, retry-eligible change with reducer-visible queued work arms a target-specific explicit-retry edge, even though its reducer command is not `RetryError`.
2. That edge causes one immediate dependency-analysis attempt even when its analysis-input signature matches the last completed input, with no queue-intent toggling and no second operator command.
3. If analysis selects the retried change and normal dependency/capacity guards allow it, the change reaches real Apply dispatch (`preparing`), proven by dispatch-start evidence rather than by an analyzer invocation count alone.
4. An accepted terminal-error retry keeps its existing behavior, including failed-classification release and Apply-budget reset; a stall-route edge performs neither.
5. The explicit-retry edge is consumed only by an eligible scheduler evaluation. A pass that ends before that evaluation — cancellation, incomplete reducer view, early break — leaves the edge armed for the next eligible evaluation; subsequent unchanged timer wakes after a real consumption remain suppressed.
6. A retry edge for one change does not release another change's failed classification and does not bypass dependency, capacity, eligibility, or retry-budget constraints.
7. Generic wake notifications without a consumed explicit-retry edge remain subject to ordinary debounce and unchanged-input suppression.

## Explicit Completion Conditions

- Every accepted retry route arms a target-specific edge at the shared arming point, and a refused or no-op retry arms none.
- Stall-route and terminal-error-route edge authority is distinguishable in code and in test: only the terminal-error route releases failed classification and Apply budget.
- A deterministic paused-time regression drives the **production** loop path — the reanalysis reason is derived from the consumed edge, not supplied by the test — starting from a completed matching signature and an accepted retry, and asserts a fresh analyzer invocation and a real dispatch start for the retried change.
- The same regression proves the one-shot property on later timer wakes, and proves an unconsumed edge survives a pass that ends before analysis.
- The regression fails if the edge is never armed, if the reason is reduced to an ordinarily suppressible wake, or if dispatch is stubbed to a no-op.
- Existing explicit-edge, F5-retry, and unchanged-input suppression tests remain green.
- `cargo test --lib accepted_retry_publishes_explicit_retry_edge` and `cargo test --lib retry_change_bypasses_unchanged_analysis_input_and_dispatches` pass.

## Out of Scope

- Changing the analysis-input signature contents.
- Disabling unchanged-input suppression for ordinary timer or generic wake paths.
- Altering retry eligibility, retry-route classification inputs, dependency ordering, capacity limits, or repair-budget policy.
- Extending failed-classification or Apply-budget release to non-terminal-error routes.
- Treating queue-intent toggling as the retry mechanism.
- Durable retry-edge state across process restart; the edge remains process-local ephemeral control state.

## Premise / Context

- Requested artifact: implementation proposal plus regression coverage.
- The reducer transition and command acknowledgement already succeed; the missing outcome is scheduler dispatch.
- `src/parallel/orchestration.rs` and `src/parallel/queue_state.rs` already define explicit edges as one-shot bypasses of unchanged-input suppression; `src/orchestration/operator_command.rs` decides which accepted retry ever arms one.
- The existing `src/parallel/tests/unchanged_analysis_input.rs` harness takes the reanalysis reason as a caller-supplied parameter, so on its own it cannot witness this defect; the production-path regression must start from the accepted retry command.
- The fix must preserve the constitution's workspace-local durable routing and use only ephemeral process-local edge state.
