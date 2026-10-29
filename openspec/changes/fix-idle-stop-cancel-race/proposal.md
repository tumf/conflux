---
change_type: implementation
priority: low
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/orchestration.rs
  - src/parallel/dependency.rs
  - src/orchestration/run_control.rs
verifications:
  - id: idle-stop-cancel-race
    requirement: Cancellation completed during the actual blocked-only classification await prevents settlement, while a pending or later stop still settles exactly once
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/blocked_idle_graceful_stop.rs
    evidence: cargo test --locked --offline --lib --features web-monitoring blocked_idle_graceful_stop -- --nocapture
    rerun: cargo test --locked --offline --lib --features web-monitoring blocked_idle_graceful_stop -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: idle-stop-existing-boundaries
    requirement: The existing empty-boundary, no-request, and shared lifecycle stop contracts remain intact
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/command_handlers/idle_start_running_tests.rs
    evidence: cargo test --locked --offline --lib --features web-monitoring idle_stopping_ -- --nocapture
    rerun: cargo test --locked --offline --lib --features web-monitoring idle_stopping_ -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Honor cancel-stop after blocked-idle classification

**Change Type**: implementation

## Problem / Context

On develop `0444fd4c4f32be306f072619a1d2017393d457cb`, `settle_graceful_stop_when_no_work` reads the shared graceful-stop flag at entry, awaits the blocked-only classifier, then emits `Stopped` without reading the flag again (`src/parallel/orchestration.rs:334-361`). Shared run control can accept cancel-stop and clear that same flag during the await (`src/orchestration/run_control.rs:1058-1066`). The scheduler then stops a request already withdrawn by the operator.

This is a source-confirmed, low-severity residual race in the recently integrated blocked-idle stop fix, not a new Stop-wake defect. The real suspension route is an archived dependency's workspace/base-evidence lookup: `queue_state.rs:3626-3629,3697-3716` -> `dependency.rs:194-206,217-242`. The passed reducer snapshot is already captured; there is no reducer-lock acquisition inside this classifier. A merely missing dependency takes a synchronous classification path. Existing `blocked_idle_graceful_stop_cancel_before_boundary_never_stops` sets then clears the flag before waking a pass and therefore does not exercise this interleaving.

Investigation here is source inspection, not a claim that the new regression or fix has already run. This authoring change contains proposal artifacts only; Conflux implements the following runtime contract separately.

## Proposed Solution

Retain the existing entry check and both existing safe-boundary predicates. After a boundary qualifies, re-read the existing shared graceful-stop flag immediately before beginning the terminal `Stopped` send. If it is false, return `false` without a terminal event or a loop-exit decision. No extra await may be introduced between that final read and starting the existing send. Keep successful-settlement logging after the final read so a withdrawn request is not logged as settled.

The contract is deliberately narrow: a cancel-stop that completed while blocked-only classification was pending, and whose flag is still false at the final read, wins this evaluation. A new Stop after cancellation is a new pending request, not a request to suppress forever. The existing asynchronous channel send and command/event dispatch ordering are unchanged; this proposal does not promise atomic cancellation after that final read or while a terminal event send is already pending.

## Change Boundary and Preserved Contracts

- Runtime change: only `settle_graceful_stop_when_no_work` in `src/parallel/orchestration.rs` and its comments. Tests: `src/parallel/tests/blocked_idle_graceful_stop.rs`, with narrowly necessary test-only support in `src/parallel/tests/support.rs` if shared construction requires it. Use existing `set_workspace_manager`, shared stop flag, and run-control test ports; no production seam is required.
- Preserve the coherent snapshot requirement and unchanged blocked-only classification. Do not settle with in-flight/JoinSet work, resolve/reject lane waiters, manual resolver, pending merge, or cleanup obligations.
- Preserve the fully drained boundary; no-request and withdrawn requests never settle. No changes to Start/Retry, stop/force-stop eligibility, event payloads, queue reconciliation, marks, held evidence, or frontend projections.
- Preserve exactly one terminal event per successful loop settlement, the existing `graceful_stop_settled` return path, and no `AllCompleted`, work dispatch, upstream finalization, or early cleanup on this path.
- Classification false or incomplete evidence still returns false. Existing dependency-evidence error behavior is not changed; cancellation is checked after whatever existing classification returns.
- No new runtime flag, setting, timeout, durable state, lock, generation counter, public API, UI design, dependency, or migration.

## Acceptance Criteria

1. A deterministic regression enters the actual production settlement helper with a true request and complete snapshot, then suspends inside the real blocked-only classifier's workspace call. Only after an entered-channel acknowledgement does real `RunControlService::cancel_stop(OperatorMode::Stopping)` return `StopCancelled` and clear the executor's shared flag. Releasing the workspace barrier yields `false` and zero `Stopped`/`AllCompleted` events.
2. With identical genuinely blocked repository evidence but no cancellation during the barrier, settlement returns true and publishes exactly one `Stopped`, never success or work-start events. After a cancelled evaluation, a later accepted Stop still settles normally; no cancellation latch is added.
3. Existing full-loop positive, pre-entry-cancellation, owed-work, incomplete-snapshot, dispatchable-candidate, and empty-boundary tests stay passing. New boundary tests do not replace the real scheduler-loop coverage or claim to prove wake wiring on their own.
4. The declared focused commands select nonzero nonignored cases, including the two named new regressions in `design.md`. No sleep, elapsed-time threshold, timeout-as-success, synthetic `Stopped`, or fake classifier result is valid evidence.

## Explicit Completion Conditions

Both declared verification commands pass once from the candidate with all required new and existing tests selected. The cancellation regression must fail on the pre-fix implementation because the helper returns true/emits `Stopped`, not because a test barrier was never reached. Review the guard location and unchanged loop-exit/cleanup paths alongside the test evidence. Retain results outside the target worktree; do not add operational reports or a change-directory runner.

The tracked `.pre-commit-config.yaml` Rust hooks are staged-path-scoped, not unconditional; they do not replace these focused executable gates. Native strict proposal validation and archive validation remain separate from runtime test evidence. No deployed-service outcome is claimed.

## Dependencies / Non-goals

The blocked-idle boundary and test surfaces are already integrated in `0444fd4c`; no active change dependency is needed. This is one inseparable guard-plus-regression change, independent of unrelated library fixture repairs.

Do not redesign stop linearization, backpressured event delivery, active-work graceful-stop policy, dependency classification, analysis activity display, or cancel-stop UI. Do not operate/restart a live owner, publish, install a build, merge the base branch, or modify another candidate as part of this change.
