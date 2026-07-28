---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/manual_resolve.rs
  - src/parallel/tests/auto_resolve.rs
verifications:
  - id: scheduler-local-tests
    requirement: Resolve-completion re-analysis is edge-triggered without breaking immediate dependency refresh or capacity recovery dispatch
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for targeted parallel scheduler regression tests
    rerun: cargo test parallel::tests --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent repeated resolve-completion analysis

**Change Type**: implementation

## Problem / Context

A live `cflx` run in the `latch` repository repeatedly launched dependency-analysis agents without completing or adding work. The scheduler log showed the same `iteration=2`, `trigger=resolve_completion`, queued set, and zero-dispatch-capacity state every cycle.

`wait_for_scheduler_event` records `ReanalysisReason::ResolveCompletion` when a workspace or background merge completes. The next scheduler loop correctly consumes that event to run immediate dependency analysis. When capacity remains zero, dispatch is suppressed and the scheduler waits again. The 500 ms timer branch does not replace the already-consumed reason, so the following loop reuses `ResolveCompletion`. Because that reason bypasses queue debounce, each timer wake starts another expensive LLM analysis.

The existing capacity-zero diagnostic deduplication suppresses repeated operator logs but does not suppress the repeated analysis calls themselves.

## Proposed Solution

Treat edge-triggered debounce-bypass reasons as one-shot scheduler events rather than persistent loop state.

- Preserve one immediate dependency-analysis evaluation after each actual resolve, workspace, merge completion, repair-candidate addition, or slot-recovery edge.
- Consume an edge-triggered reason only after the scheduler actually enters its reanalysis/dispatch evaluation with queued work; do not consume it in a loop where the evaluation block is skipped.
- Reset the loop-owned reason to the existing non-bypass `Initial` state after evaluation so timer-only wakes cannot reuse it.
- Let later timer-only wakes follow the existing bounded queue-debounce policy unless a new explicit bypass event occurs.
- Preserve immediate analysis and dispatch when a new completion or capacity-recovery event occurs.
- Audit every path that can decrease manual/automatic resolve or pending merge capacity and prove that it either produces a scheduler wake edge or remains recoverable through bounded timer/debounce evaluation.
- Keep `perform_reanalysis_and_dispatch`, capacity calculation, dependency semantics, and direct-call test meaning unchanged.
- Keep the correction in runtime scheduler state. Do not add durable workflow-control state outside the workspace.

The implementation should be the smallest scheduler-loop change that makes trigger consumption explicit. `ResolveCompletion`, `RepairCandidate`, and `SlotRecovery` share the one-shot lifetime rule because all represent state-transition edges and all bypass debounce. `QueueNotification` retains its existing candidate-addition reconciliation/reset semantics.

## Acceptance Criteria

- One actual resolve/workspace/merge completion, repair-candidate addition, or slot-recovery edge can cause at most one immediate analysis evaluation before another qualifying edge occurs.
- An edge-triggered reason is consumed only when queued work enters the scheduler's reanalysis/dispatch evaluation; a loop with no queued evaluation does not silently discard it.
- A 500 ms timer wake cannot repeatedly launch analysis by inheriting a previously consumed `ResolveCompletion`, `RepairCandidate`, or `SlotRecovery` reason.
- Zero capacity may still permit the first explicit edge-triggered analysis, while ordinary apply dispatch remains suppressed.
- Every path that restores execution capacity either wakes the scheduler through workspace/merge/queue signaling or is re-evaluated through the existing bounded timer/debounce path, so removing sticky reasons does not create starvation.
- A later completion or slot-recovery edge that restores capacity causes queued eligible work to be re-evaluated and dispatched without user action or a new queue addition.
- Queue notification and reducer reconciliation retain their existing candidate-addition debounce-bypass behavior.
- Repeated unchanged zero-capacity state does not create repeated LLM analysis attempts solely from timer wakes.
- No out-of-worktree durable workflow state is introduced.

## Explicit Completion Conditions

- `src/parallel/orchestration.rs` consumes edge-triggered reasons on the loop-owned `reanalysis_reason` after an actual queued reanalysis/dispatch evaluation and resets them to non-bypass `Initial`; `perform_reanalysis_and_dispatch` does not gain hidden trigger-consumption state.
- Repository review enumerates capacity-decreasing and capacity-recovery paths for manual resolve, automatic conflict resolution, workspace completion, pending merge completion, deferred merge retry, and failed/deferred merge results, with runnable coverage for each distinct wake/recovery mechanism.
- Loop-level regression coverage counts dependency-analysis invocations across completion/repair/slot-recovery and timer wakes, and fails if timer wakes replay a consumed edge.
- Loop-level regression coverage proves a second real edge re-arms immediate analysis, while a queued-empty loop does not consume a reason before evaluation.
- Loop-level regression coverage proves zero-capacity first analysis, apply suppression, queued retention, and later capacity-recovery dispatch.
- Existing direct-call manual-resolve, auto-resolve, resolve-completion dispatch, queue debounce bypass, repair-candidate, slot-recovery, and diagnostic-deduplication tests retain their original meaning and continue to pass without weakened expectations.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, and the relevant default-path Rust tests pass.

## Scope Completeness

- User-visible outcome: Conflux stops consuming agents and CPU for repeated no-progress analysis while preserving autonomous scheduler progress.
- Likely code area: scheduler event/reanalysis-reason handling in `src/parallel/orchestration.rs`, with tests under `src/parallel/tests/`.
- Verification: repository-local unit/integration-style scheduler tests that detect no-op implementations by asserting analyzer invocation counts and dispatch state.
- Migration and rollout: none; the state is runtime-only and existing runs restart with the corrected scheduler behavior.
- Follow-up work: renaming `ResolveCompletion` or changing the existing workspace-completion-to-reason mapping is intentionally deferred because it changes trigger classification rather than trigger lifetime.

## Out of Scope

- Disabling all analysis while execution capacity is zero.
- Changing dependency classification, LLM analysis prompts, or analysis result parsing.
- Changing the 500 ms scheduler timer duration.
- Redesigning scheduler event transport or adding durable event-consumption state.
- Renaming `ResolveCompletion`, changing which workspace completions currently map to it, or changing the existing `Completion` versus `ResolveCompletion` classification.
- Modifying diagnostic deduplication except where a test requires alignment with the corrected invocation behavior.
