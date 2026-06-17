---
change_type: implementation
priority: high
dependencies: []
references:
  - "src/parallel/queue_state.rs:1876-2169 (reducer-visible queue reconciliation)"
  - "src/parallel/queue_state.rs:2528-2825 (reanalysis and dispatch loop)"
  - "src/parallel/dynamic_queue.rs:19-53 (queue debounce gate)"
  - "src/parallel/orchestration.rs:220-235 (queue reconciliation trigger handling)"
  - "openspec/specs/parallel-execution/spec.md:609-683 (Parallel Analysis Targeting)"
---

# Fix reducer queue debounce starvation

**Change Type**: implementation

## Premise / Context

- A running Conflux scheduler was observed with changes stuck across blocked, queued, and merged-looking states while no new `analyze_command` execution occurred.
- The log showed `Queue reconciliation adding reducer-queued change candidate: unify-dependency-classification` followed immediately by `Debounce period active (0.0s < 10s), deferring re-analysis` on repeated scheduler ticks.
- The root cause is that reducer-visible queued reconciliation can refresh `last_queue_change_at` every loop, so the 10-second debounce window never elapses.
- The fix must preserve the existing distinction between explicit queue additions, debounceable timer wakes, blocked-only drain, and notification-driven persistent idle behavior.
- The Conflux constitution requires workflow control state to remain derivable from workspace/git/base state, so any new tracking must not introduce durable out-of-worktree routing state.

## Problem / Context

Reducer-visible queued work exists when the shared orchestrator state records a change as queued, even if the scheduler-local `queued` vector must reconstruct it from OpenSpec files or workspace evidence.

Today, each successful reducer reconciliation addition increments the queue-addition outcome and refreshes the queue debounce timestamp. In a long-running scheduler where the same reducer-visible candidate is repeatedly reintroduced before analysis, this can keep the debounce elapsed time near zero forever.

The user-visible symptom is a run that remains `running`, with changes visible as blocked, queued, or merged, while dependency analysis no longer starts.

## Proposed Solution

Update reducer-visible queue reconciliation so repeated reconstruction of the same reducer-visible queued intent does not continuously refresh the queue debounce timestamp.

The scheduler should still stamp debounce state for the first reducer-visible queued addition when no queue-change timestamp exists, and dynamic queue ingestion should continue to represent true fresh operator queue edits. However, reconciliation must not treat every timer-driven rediscovery of already-known reducer state as a new queue edit.

This is a targeted scheduler correctness fix, not a broader refactor of dependency classification or dispatch guard structure.

## Acceptance Criteria

- A reducer-visible queued candidate that is repeatedly reconciled across scheduler ticks cannot keep resetting debounce elapsed time to zero.
- A fresh reducer-visible queued candidate still becomes scheduler-local queued work and can trigger analysis according to the existing queue-notification policy.
- Dynamic queue additions and explicit queue notifications continue to bypass debounce where the current spec already requires it.
- Blocked-only states continue to skip analyzer execution and do not become false dispatchable work.
- The fix uses runtime-only in-memory debounce/diagnostic state and does not add durable workflow-control state outside the workspace/git/base evidence model.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` changes reducer-visible reconciliation timestamp handling so an existing debounce timestamp is not refreshed by repeated reconciliation alone.
- `src/parallel/tests/executor.rs` includes a regression test that initializes an existing debounce timestamp, reconciles reducer-visible queued work, and asserts the timestamp is preserved.
- Existing coverage for first reducer-visible queued addition still asserts the scheduler records an initial debounce timestamp when none exists.
- Targeted regression tests for reducer-visible queue reconciliation pass.
- Repository default Rust checks pass: `cargo fmt --check`, `cargo test`, and `cargo clippy -- -D warnings`.

## Out of Scope

- Changing the 10-second debounce duration.
- Changing dependency classification rules for archived, rejected, queued, in-flight, or missing dependencies.
- Refactoring `perform_reanalysis_and_dispatch`; that is covered by `extract-reanalysis-dispatch-guards`.
- Fixing stale/prunable external worktree entries such as `/Users/tumf/tmp/conflux-pr12-review`.
