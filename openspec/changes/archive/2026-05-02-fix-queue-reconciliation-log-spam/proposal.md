---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/observability/spec.md
  - openspec/changes/archive/2026-05-02-fix-queued-analysis-reconciliation/proposal.md
  - openspec/changes/archive/2026-05-02-fix-queued-analysis-reconciliation/design.md
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/executor.rs
---

# Change: Fix queue reconciliation log spam

**Change Type**: implementation

## Premise / Context

- A live Conflux run repeatedly emitted `Queue reconciliation deferred for '<change_id>': already_active` while changes were active or in-flight.
- The emitting code path is `reconcile_queued_candidates_from_shared_state()` in `src/parallel/queue_state.rs`, called every scheduler loop from `src/parallel/orchestration.rs`.
- The archived `fix-queued-analysis-reconciliation` change intentionally made no-analysis reasons observable, including `already_active`, but did not require the same healthy transient reason to be logged every loop.
- `openspec/CONSTITUTION.md` allows logs, metrics, caches, and UI state only as non-authoritative observability outputs; any dedupe state must not influence workflow control.

## Requested Artifact

- Implementation proposal to suppress or rate-limit repetitive queue reconciliation diagnostics without changing scheduling behavior.
- Regression coverage that proves active/in-flight queued intent is still not duplicated into scheduler-local queued candidates.
- Observability behavior that keeps actionable diagnostics available without flooding TUI logs.

## Problem

Queue reconciliation is designed to recover reducer-visible queued intent into scheduler-local analysis candidates. When a reducer-visible queued intent points at a change that is already active or in-flight, the scheduler correctly defers adding it to the local queue. However, the current implementation emits a user-visible info log for every scheduler loop iteration while that condition remains true.

This makes the TUI Logs View noisy and can obscure useful agent output or actionable diagnostics. The repeated message does not indicate new state, user action required, or scheduler failure; it is a stable healthy guardrail preventing duplicate dispatch.

## Proposed Solution

Introduce observability deduplication for queue reconciliation diagnostics while preserving scheduler behavior:

1. Treat `already_active` as a stable transient diagnostic that must not be emitted as a repeated user-visible info log on every scheduler loop.
2. Keep the existing duplicate-dispatch protection: active or in-flight reducer-visible queued changes must not be added to scheduler-local queued candidates.
3. Preserve observability by emitting the first occurrence or a bounded summary/rate-limited message for repeated `(change_id, reason)` diagnostics.
4. Keep more actionable reasons such as `candidate_not_found` visible, but deduplicate identical consecutive occurrences so they cannot flood logs.
5. Store any dedupe/rate-limit bookkeeping only in memory and use it exclusively for logging decisions, never as workflow-control input.

## Acceptance Criteria

1. Repeated scheduler loops with the same active/in-flight reducer-visible queued change do not emit `Queue reconciliation deferred ... already_active` to TUI Logs View on every loop.
2. The first occurrence or a periodic summary still makes the `already_active` reconciliation reason observable for debugging.
3. `candidate_not_found` remains observable when a reducer-visible queued change cannot be loaded from active OpenSpec changes, but identical repeated messages are deduplicated or rate-limited.
4. Queue reconciliation behavior is unchanged: active/in-flight changes are not duplicated into scheduler-local queued candidates, and the same change is recoverable after active/in-flight state clears.
5. Any dedupe state is runtime-ephemeral and non-authoritative, consistent with `openspec/CONSTITUTION.md`.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` or adjacent scheduler observability code suppresses or rate-limits repeated `already_active` queue reconciliation log events by `(change_id, reason)` or an equivalent stable key.
- The implementation does not change the conditions used to decide whether a reducer-visible queued change is added to scheduler-local `queued`.
- Regression tests in `src/parallel/tests/executor.rs` or a focused adjacent test module prove both non-duplication while active/in-flight and recovery after release still work.
- Tests or explicit manual verification prove repeated reconciliation of the same active/in-flight change emits at most the allowed bounded user-visible diagnostic output.
- OpenSpec strict validation passes for this proposal.
- Repository verification passes for formatting and the targeted Rust tests touching queue reconciliation behavior.

## Out of Scope

- Changing dependency analysis prompts, ordering, or dispatch selection.
- Changing queue intent semantics, reducer active-state semantics, or lifecycle status labels.
- Removing queue reconciliation diagnostics entirely.
- Introducing persistent log suppression files, external caches, or any out-of-worktree workflow-control state.
