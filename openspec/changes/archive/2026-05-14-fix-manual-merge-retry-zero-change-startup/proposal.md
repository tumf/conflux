---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/command_handlers.rs
  - src/tui/orchestrator.rs
  - src/parallel_run_service.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/executor.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/tui-resolve/spec.md
---

# Fix manual merge retry zero-change scheduler startup

**Change Type**: implementation

## Problem / Context

After a parallel change reaches `merge wait`, pressing `M` should register reducer-owned `ResolveWait` retry intent and let the scheduler consume that retry. In practice, a restarted TUI can accept the `M` key, show `resolve pending`, start a manual resolve scheduler with zero normal changes, and then immediately report success:

- `Scheduled merge-wait retry intent for '<change>'; started scheduler for manual resolve`
- `Starting parallel processing of 0 change(s)`
- `Execution completed (0 changes processed)`
- `All parallel changes completed`

This leaves the change stuck in `resolve pending` without `resolving`, `merged`, `merge wait`, or a visible error. The suspected failure path is that empty manual resolve startup preserves retry intent in the TUI reducer, but the scheduler/executor path does not reliably observe the same shared reducer state or treats an empty normal queue as completion before consuming lane-wait retry work.

## Proposed Solution

Fix this as a state-ownership and completion-semantics problem, not as another narrow retry wakeup patch:

- Define one authoritative owner for manual retry intent: the shared reducer state that accepted `ReducerCommand::ResolveMerge`.
- Preserve that caller-provided shared reducer state through `ParallelRunService` and `ParallelExecutor` when TUI starts an empty manual resolve scheduler.
- Treat executor-local retry sets as synchronized caches only; they must not outlive or contradict reducer-owned membership.
- Ensure a scheduler started with no normal changes but with reducer-owned `ResolveWait` / `RejectWait` enters the retry dispatch path.
- Prevent success completion and `AllCompleted` emission while reducer-owned lane-wait retry work remains pending or active.
- Convert stale or missing retry prerequisites into visible terminal/manual states instead of silent `resolve pending` persistence.
- Strengthen regression tests so empty manual resolve startup proves actual retry dispatch or visible demotion, not merely absence of start rejection.
- Protect existing non-empty queue dispatch, active resolve deferral, and dirty-base demotion behavior with regression coverage to avoid side effects.

## Acceptance Criteria

- Pressing `M` on a `merge wait` row after cflx restart starts scheduler-owned retry work, not a zero-change no-op success.
- The executor used by the restarted manual resolve scheduler observes the same reducer-owned `ResolveWait` membership that accepted `ReducerCommand::ResolveMerge`.
- A zero-change manual resolve run must not emit successful completion / `AllCompleted` while shared reducer state still contains `ResolveWait` or `RejectWait` membership.
- If retry prerequisites are clean and a valid archived workspace exists, the change progresses to `resolving` and then `merged` or a visible failure state.
- If retry prerequisites are dirty, stale, or missing, the change returns to `merge wait` or an explicit visible error/stalled state with a reason; it must not remain indefinitely in `resolve pending`.
- Existing non-empty parallel queue dispatch and active-scheduler manual retry behavior remain unchanged.
- The fix removes split-brain retry ownership: retry intent is accepted, consumed, cleared, and displayed from the same reducer-owned lifecycle rather than from diverging TUI/executor-local state.
- Completion remains truthful under the Conflux constitution: no run is treated as implemented, complete, or archive-ready based on hidden runtime state or a zero-change shortcut while observable retry work remains.

## Explicit Completion Conditions

Implementation is complete when repository evidence shows:

- `src/parallel_run_service.rs` no longer overwrites caller-provided shared reducer state during manual resolve scheduler startup.
- `src/tui/orchestrator.rs` completion handling is aware of reducer-owned retry work and does not report zero-change success while lane-wait retry work remains.
- `src/parallel/orchestration.rs` / `src/parallel/queue_state.rs` dispatch reducer-owned lane-wait retry work for empty manual startup before idle completion.
- Tests cover the restarted zero-change manual resolve path and fail if the scheduler exits as a 0-change success without consuming retry intent.
- `cargo test` targets covering parallel scheduler, TUI command handling, and orchestration state pass.
- Regression tests explicitly cover side-effect-sensitive paths: active scheduler notification, non-empty queue dispatch, auto-resumable deferral, dirty-base demotion, stale workspace handling, and merged-state non-regression.

## Out of Scope

- Changing the `M` key into a direct merge execution path.
- Introducing durable out-of-worktree workflow state.
- Redesigning parallel dependency analysis or normal queued change dispatch.
- Changing serial-mode archive semantics.
