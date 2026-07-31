## MODIFIED Requirements

### Requirement: Parallel execution completion status must accurately reflect actual processing outcome

The system SHALL send completion events and messages only when processing completes normally, not when stopped or cancelled by the user. The system SHALL distinguish successful completion, completion with errors, graceful stop, active-execution force stop, and scheduler-only cancellation. Operator cancellation MUST NOT be represented as an agent-command failure.

#### Scenario: Graceful stop during parallel execution should not show success message

**Given** the orchestrator is running in parallel mode
**And** at least one change is queued for processing
**When** the user triggers graceful stop before processing completes
**Then** the orchestrator stops processing after the graceful boundary
**And** sends `OrchestratorEvent::Stopped`
**And** does not send `OrchestratorEvent::AllCompleted`
**And** displays `Processing stopped` without a success completion message

#### Scenario: Force stop of active execution remains cancellation rather than failure

**Given** the orchestrator is running in parallel mode
**And** an agent command or in-flight execution is active
**When** the user triggers force stop
**Then** active execution cancellation and managed process cleanup are requested
**And** the outcome is classified as stopped or cancelled
**And** the system does not display `Execution failed: Agent command failed`
**And** the system does not display `Processing completed with errors`
**And** the system does not send `OrchestratorEvent::AllCompleted`

#### Scenario: Scheduler-only stop does not claim forceful process termination

**Given** the parallel scheduler remains alive in `MergeWait`, `ResolveWait`, deferred merge, or idle waiting
**And** no agent command or in-flight execution is active
**When** the user requests immediate stop
**Then** the scheduler/orchestrator is cancelled
**And** `Processing stopped` is displayed once
**And** no force-stop, process-termination, execution-failure, or normal-completion message is displayed
**And** `OrchestratorEvent::AllCompleted` is not sent

#### Scenario: Successful parallel execution completion shows success message

**Given** the orchestrator is running in parallel mode
**And** multiple changes are queued for processing
**When** all changes complete successfully without errors or cancellation
**Then** the orchestrator sends `OrchestratorEvent::AllCompleted`
**And** displays the existing successful completion messages

#### Scenario: Parallel execution with genuine errors shows warning message

**Given** the orchestrator is running in parallel mode
**When** a non-cancellation execution error occurs
**And** all eligible queued work has been attempted
**Then** the orchestrator sends `OrchestratorEvent::AllCompleted`
**And** displays `Processing completed with errors`
**And** does not display a successful completion message

### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination as normal completion, genuine execution error, graceful stop, active-execution force stop, scheduler-only cancellation, or merge wait. This termination reason SHALL control terminal logs and events without inferring process activity from TUI mode or error-message text. Operator cancellation SHALL request cancellation without dropping the running scheduler future and SHALL establish terminal stop only after the scheduler reaches its bounded cleanup barrier, including active task drain and pending background merge/base-lane result handling.

#### Scenario: Operator cancellation reaches terminal classification

**Given** the global parallel cancellation token is triggered by an operator stop
**When** the outer parallel orchestration boundary observes cancellation before the scheduler future returns
**Then** the termination reason is recorded as stopped or cancelled
**And** cancellation is not converted to `OrchestratorError::AgentCommand`
**And** the outer boundary continues polling the scheduler future until its bounded cleanup barrier completes
**And** active task drain, registered execution-handle cleanup, and pending merge/base-lane result handling precede terminal stop
**And** a cleanup deadline or managed escalation remains classified as operator cancellation rather than execution failure
**And** later terminal event handling remains idempotent if the frontend already applied `OrchestratorEvent::Stopped`
**And** `Processing stopped` is not logged more than once

#### Scenario: Genuine failure remains distinct

**Given** a parallel service or command fails without operator cancellation
**When** the outer parallel orchestration boundary handles the result
**Then** the termination reason is recorded as genuine execution error
**And** existing failure and completion-with-errors reporting remains enabled
