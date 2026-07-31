---
change_type: implementation
priority: high
dependencies: []
references:
  - "openspec/CONSTITUTION.md"
  - "openspec/specs/cli/spec.md"
  - "openspec/specs/parallel-execution/spec.md"
  - "src/tui/key_handlers.rs"
  - "src/tui/command_handlers.rs"
  - "src/tui/orchestrator.rs"
  - "src/tui/state/event_handlers/processing.rs"
  - "src/parallel/orchestration.rs"
  - "src/tui/queue.rs"
verifications:
  - id: idle-parallel-stop-local
    requirement: TUI stop classification and parallel cancellation completion are covered by repository-local regression tests.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/orchestrator.rs
    evidence: matching idle_parallel_stop tests exist and pass, together with formatting and lint output
    rerun: cargo test --lib idle_parallel_stop -- --list | grep -q idle_parallel_stop && cargo test --lib idle_parallel_stop && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: classify idle parallel stops truthfully

**Change Type**: implementation

## Problem / Context

A parallel TUI run can remain alive after a change has archived and entered `MergeWait`, even though no AI agent child process is running. The first Esc changes the TUI to `Stopping`; a second Esc currently treats that mode alone as proof that forceful process termination is required.

In the observed run, the second Esc arrived after `Archived` and `MergeWait`. The TUI emitted `Processing stopped` and `Force stopped`, then the outer parallel cancellation branch converted the operator cancellation into `OrchestratorError::AgentCommand`, followed by `Execution failed` and `Processing completed with errors`. No process-group termination or termination failure appeared in the runtime log.

The existing CLI requirement correctly guarantees termination of an active agent process and descendants. It does not require an idle scheduler or merge-wait state to claim that a process was force-terminated. The canonical parallel completion requirement also requires cancellation to remain distinct from execution failure.

## Proposed Solution

Classify a second-Esc stop from current runtime activity rather than from `AppMode::Stopping` alone.

- Preserve the current force-stop path when an in-flight execution owns active cancellation/process activity; request cancellation and retain the existing child-process cleanup guarantee.
- When no in-flight execution or active process handle exists, cancel the still-live scheduler/orchestrator without claiming process termination or force stop.
- Treat both operator stop outcomes as cancellation/stopped outcomes in the outer parallel orchestrator, not as `AgentCommand` failures.
- Emit one truthful terminal stop message and suppress `Execution failed`, `Processing completed with errors`, success completion messages, and `AllCompleted` for operator cancellation.
- Keep execution marks and reset transient queued/in-flight presentation according to the existing Stopped-mode policy.

The stop classification, UI message, and outer completion classification must ship together: changing only one would leave either process cleanup, operator feedback, or terminal events contradictory.

## Acceptance Criteria

1. A second Esc while an agent command or in-flight execution is active requests cancellation through the existing managed cleanup path and does not weaken descendant-process termination guarantees.
2. A second Esc while the parallel orchestrator is alive but no execution/process activity exists stops scheduler/orchestrator waiting without attempting or claiming forceful process termination.
3. The idle case displays `Processing stopped` once and does not display `Force stopped` or any process-termination claim.
4. User cancellation does not produce `Execution failed: Agent command failed`, `Processing completed with errors`, success completion logs, or `OrchestratorEvent::AllCompleted`.
5. Genuine non-cancellation execution failures continue to produce the existing execution-failure and completion-with-errors behavior.
6. Stopped-mode queue reset and execution-mark preservation remain unchanged.

## Explicit Completion Conditions

- The second-Esc decision consumes explicit current execution activity evidence; `AppMode::Stopping` alone is not sufficient to classify a force stop.
- Both direct TUI key handling and command-dispatch stop paths use equivalent classification and terminal messaging.
- The parallel orchestration result represents operator cancellation separately from command failure and sets cancellation completion state before terminal event/message selection.
- Repository-local tests cover active force stop, idle `MergeWait`/deferred-merge stop, cancellation message suppression, genuine error preservation, and stopped-state mark preservation.
- `cargo test --lib idle_parallel_stop`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and strict OpenSpec validation pass.

## Out of Scope

- Changing the first-Esc graceful-stop contract.
- Removing force stop for an actually active agent command.
- Changing per-change `stop_and_dequeue` behavior.
- Adding durable process/activity state outside the workspace.
- Redesigning serial-mode stop semantics or remote server shutdown.
