---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/key_handlers.rs
  - src/tui/state.rs
  - src/tui/command_handlers.rs
  - src/tui/queue.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/dynamic_queue.rs
  - src/parallel/tests/executor.rs
  - src/parallel/tests/manual_resolve.rs
  - openspec/specs/parallel-execution/spec.md
---

# Fix Queue Notification Reanalysis

**Change Type**: implementation

## Problem / Context

During a running TUI parallel execution, pressing `x` in the Changes view can mark `not queued` changes for queueing, but dependency analysis may not start promptly afterward. The relevant runtime path is `x` key handling through `AppState::toggle_all_marks()`, `TuiCommand::AddToQueue`, `DynamicQueue::push()`, scheduler dynamic queue ingestion, and `ParallelExecutor::perform_reanalysis_and_dispatch()`.

Current scheduler logic records queue ingestion in `last_queue_change_at`, but later treats `ReanalysisReason::QueueNotification` as debounceable. After the first scheduler iteration, this can suppress analysis for an explicit user queue action, especially while other work or manual resolve state is active. That violates the intended operator experience and the canonical `Parallel Analysis Targeting` requirement that re-analysis can start from queue changes and can run even when dispatch capacity is zero.

## Proposed Solution

Treat explicit queue notifications that introduce scheduler-visible queued work as immediate re-analysis triggers, not timer/poll debounce candidates.

The implementation should preserve debounce for repeated timer-driven checks and no-state-change loops, but bypass debounce when:

- `DynamicQueue` ingestion adds one or more loadable changes to scheduler-local `queued` work.
- reducer-visible queue reconciliation adds one or more loadable queued candidates.
- the queue notification arrives while dispatch capacity is zero, in which case analysis still runs and dispatch remains capacity-gated.

The fix must not introduce durable workflow-control state outside the workspace. Any new scheduler flags or attempt markers must remain in-memory observability/control-loop state only.

## Acceptance Criteria

- Pressing `x` during Running mode on `not queued` rows emits queue commands and results in prompt scheduler analysis once the queued candidates are ingested.
- A `QueueNotification` that adds new queued work is not suppressed by the 10-second queue debounce window.
- Analysis still runs when all ordinary dispatch slots are occupied or held by manual/resolve work, but apply dispatch remains suppressed until capacity is available.
- Queue batching remains sane: bulk `x` additions should be ingested together where possible and should not create one analysis per individual row when the scheduler can process the batch in one cycle.
- Existing blocked-only and persistent-idle behavior remains intact: no new timer-driven worktree/repository polling is introduced while the scheduler is blocked-only drained or fully idle.
- The fix is covered by regression tests that fail if `QueueNotification` remains debounce-blocked.

## Explicit Completion Conditions

This change is complete only when repository evidence shows all of the following:

- `src/parallel/queue_state.rs` or adjacent scheduler code distinguishes explicit queue additions from debounceable timer/poll rechecks.
- Tests prove that `iteration > 1` plus fresh `last_queue_change_at` plus `ReanalysisReason::QueueNotification` still emits `AnalysisStarted` for newly queued work.
- Tests prove that a running/persistent scheduler receiving dynamic queue work after initial analysis emits `AnalysisStarted` without waiting 10 seconds.
- Tests prove that zero dispatch capacity does not block analysis for the queue notification path, while no `ApplyStarted` event is emitted until capacity exists.
- `cargo test` targeted at the affected parallel/TUI tests passes, and default quality gates used by this repository pass or any heavy tests are intentionally excluded per AGENTS.md.

## Out of Scope

- Replacing the queue debounce mechanism globally.
- Reworking TUI selection semantics beyond the queue command path needed for this bug.
- Changing durable workflow-control state or adding persistent queue state outside workspace/git evidence.
- Changing archive, acceptance, or merge correctness semantics unrelated to queue-triggered reanalysis.
