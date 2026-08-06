## MODIFIED Requirements

### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination as normal completion, genuine execution error, graceful stop, active-execution force stop, scheduler-only cancellation, merge wait, or run-fatal failure. This termination reason SHALL control terminal logs and events without inferring process activity from TUI mode or error-message text. Operator cancellation SHALL request cancellation without dropping the running scheduler future and SHALL establish terminal stop only after the scheduler reaches its bounded cleanup barrier. That barrier MUST include active workspace-task drain, a truthful registered execution-handle outcome (confirmed completion or bounded unconfirmed timeout), invocation-scoped AI runner-task and owned process-set quiescence or completed managed escalation, and pending background merge/base-lane result handling. Aborting workspace futures or removing execution handles MUST NOT by itself establish process quiescence. Run-fatal failure SHALL retain exactly one prompt global `Error` before cleanup waiting, stop new admission and retries, and return scheduler failure only after the same cleanup barrier.

#### Scenario: Operator cancellation reaches terminal classification

**Given** the global parallel cancellation token is triggered by an operator stop
**And** one or more run-owned AI commands may be active, waiting to spawn, or waiting to retry
**When** the outer parallel orchestration boundary observes cancellation before the scheduler future returns
**Then** the termination reason is recorded as stopped or cancelled
**And** run command admission closes before workspace futures are aborted
**And** cancellation is not converted to `OrchestratorError::AgentCommand`
**And** the outer boundary continues polling the scheduler future until its bounded cleanup barrier completes
**And** active task drain, a truthful execution-handle outcome, pending merge/base-lane result handling, and either confirmed runner-task and owned process-set cleanup or completed managed escalation precede terminal stop
**And** no retry or command process starts after scope shutdown
**And** a cleanup deadline or managed escalation remains classified as operator cancellation rather than execution failure
**And** later terminal event handling remains idempotent if the frontend already applied `OrchestratorEvent::Stopped`
**And** `Processing stopped` is not logged more than once

#### Scenario: Run-fatal Error remains prompt while failure return waits for cleanup

**Given** a typed background base-lane outcome invalidates the run
**And** a run-owned AI command or retry task is still active
**When** the queue boundary classifies the outcome as run-fatal
**Then** it emits exactly one global `Error` promptly without waiting for process cleanup
**And** it closes run command admission and signals active runner tasks
**And** no new ordinary dispatch or retry is started
**And** the scheduler does not return its run-fatal failure until registered runner tasks and owned process sets reach the bounded cleanup barrier
**And** Conflux-owned preparation or workspace cleanup does not race the still-registered command
**And** the run emits neither `Stopped` nor `AllCompleted` for the run-fatal outcome

#### Scenario: Aborted workspace future does not prove execution completion

**Given** a per-change workspace future has a registered execution handle and a run-owned AI command
**When** cancellation or run-fatal shutdown aborts the workspace future
**Then** removing the workspace future does not fire the execution `done` handshake by itself
**And** confirmed terminal command cleanup precedes `done`
**And** unconfirmed cleanup remains pending for bounded timeout and managed escalation rather than reporting false completion

#### Scenario: Genuine failure remains distinct

**Given** a parallel service or command fails without operator cancellation
**When** the outer parallel orchestration boundary handles the result
**Then** the termination reason is recorded as genuine execution error
**And** existing failure and completion-with-errors reporting remains enabled
