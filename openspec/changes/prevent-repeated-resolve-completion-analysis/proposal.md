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

Treat `ResolveCompletion` as an edge-triggered scheduler event rather than a persistent loop state.

- Preserve one immediate dependency-analysis evaluation after each actual resolve, workspace, or merge completion event.
- Consume the completion reason after that evaluation so timer-only wakes cannot reuse it.
- Let later timer-only wakes follow the existing ordinary debounce policy unless a new explicit bypass event occurs.
- Preserve immediate analysis and dispatch when a new completion event releases capacity.
- Preserve existing queue-notification, repair-candidate, and slot-recovery behavior.
- Keep the correction in runtime scheduler state. Do not add durable workflow-control state outside the workspace.

The implementation should be the smallest scheduler-loop change that makes trigger consumption explicit. Capacity calculation and dependency semantics should remain unchanged unless tests prove a narrowly required adjustment.

## Acceptance Criteria

- One actual resolve or merge completion can cause at most one immediate `ResolveCompletion` analysis evaluation before another qualifying completion event occurs.
- A 500 ms timer wake cannot repeatedly launch analysis by inheriting a previously consumed `ResolveCompletion` reason.
- Zero capacity may still permit the first completion-triggered analysis, while ordinary apply dispatch remains suppressed.
- A later completion that restores capacity causes queued eligible work to be re-evaluated and dispatched without user action or a new queue addition.
- Queue notification, reducer reconciliation, repair candidate, and slot recovery retain their existing debounce and bypass behavior.
- Repeated unchanged zero-capacity state does not create repeated LLM analysis attempts solely from timer wakes.
- No out-of-worktree durable workflow state is introduced.

## Explicit Completion Conditions

- `src/parallel/orchestration.rs` explicitly prevents a consumed completion reason from surviving into timer-only scheduler iterations.
- Regression coverage counts or otherwise observes dependency-analysis invocations across completion and timer wakes, and fails if timer wakes replay the completion trigger.
- Regression coverage proves a new completion event remains capable of triggering immediate analysis.
- Existing manual-resolve and auto-resolve zero-capacity tests continue to pass.
- Existing resolve-completion dispatch, queue debounce bypass, repair-candidate, and slot-recovery tests continue to pass.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, and the relevant default-path Rust tests pass.

## Scope Completeness

- User-visible outcome: Conflux stops consuming agents and CPU for repeated no-progress analysis while preserving autonomous scheduler progress.
- Likely code area: scheduler event/reanalysis-reason handling in `src/parallel/orchestration.rs`, with tests under `src/parallel/tests/`.
- Verification: repository-local unit/integration-style scheduler tests that detect no-op implementations by asserting analyzer invocation counts and dispatch state.
- Migration and rollout: none; the state is runtime-only and existing runs restart with the corrected scheduler behavior.
- Follow-up work: none required for this bug fix.

## Out of Scope

- Disabling all analysis while execution capacity is zero.
- Changing dependency classification, LLM analysis prompts, or analysis result parsing.
- Changing the 500 ms scheduler timer duration.
- Redesigning scheduler event transport or adding durable event-consumption state.
- Modifying diagnostic deduplication except where a test requires alignment with the corrected invocation behavior.
