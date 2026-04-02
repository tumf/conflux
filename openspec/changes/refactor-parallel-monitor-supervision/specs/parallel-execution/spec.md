## MODIFIED Requirements

### Requirement: Parallel execution completion status must accurately reflect actual processing outcome

The system SHALL send completion events and messages only when processing completes normally, not when stopped or cancelled by the user.

The system SHALL distinguish between successful completion, completion with errors, graceful stop, and cancellation.

Progress monitors (such as merge stall detection) SHALL NOT directly cancel parallel execution. Monitors SHALL report observations as events to the orchestration loop, which decides the appropriate response based on policy. By default, monitor events result in warning logs only and do not interrupt queue execution.

**Priority**: HIGH
**Rationale**: Incorrect completion messages mislead users about the processing status and can cause confusion when resuming work. Additionally, allowing monitors to directly cancel execution violates separation of concerns between observation and control, causing queue execution failures unrelated to the queue's own logic.

#### Scenario: Graceful stop during parallel execution should not show success message

**Given** the orchestrator is running in parallel mode
**And** at least one change is queued for processing
**When** the user triggers graceful stop (ESC key) before any change completes
**Then** the orchestrator should stop processing
**And** should send `OrchestratorEvent::Stopped`
**And** should NOT send `OrchestratorEvent::AllCompleted`
**And** should NOT display "All parallel changes completed" message
**And** should NOT display "All changes processed successfully" message
**And** should display "Processing stopped" message only

#### Scenario: Force stop (cancel) during parallel execution should not show success message

**Given** the orchestrator is running in parallel mode
**And** at least one change is queued for processing
**When** cancellation is triggered via cancel token
**Then** the orchestrator should immediately stop
**And** should display "Parallel execution cancelled" message
**And** should NOT send `OrchestratorEvent::AllCompleted`
**And** should NOT display any success completion messages

#### Scenario: Successful parallel execution completion shows success message

**Given** the orchestrator is running in parallel mode
**And** multiple changes are queued for processing
**When** all changes complete successfully without errors
**Then** the orchestrator should send `OrchestratorEvent::AllCompleted`
**And** should display "All parallel changes completed" success message
**And** should display "All changes processed successfully" message

#### Scenario: Parallel execution with partial errors shows warning message

**Given** the orchestrator is running in parallel mode
**And** multiple changes are queued for processing
**When** at least one batch fails with an error
**And** the orchestrator continues processing remaining changes
**And** all queued changes have been attempted
**Then** the orchestrator should send `OrchestratorEvent::AllCompleted`
**And** should display "Processing completed with errors" warning message
**And** should NOT display "All changes processed successfully" message

#### Scenario: Merge stall detection does not cancel parallel execution

**Given** the orchestrator is running in parallel mode
**And** a merge stall monitor is active
**And** the last merge commit on the base branch is older than the configured threshold
**When** the merge stall monitor detects a stall
**Then** a warning log is emitted with elapsed time and threshold
**And** a `ParallelEvent::Warning` is sent to the orchestration loop
**And** the parallel execution loop continues processing queued changes
**And** the shared `CancellationToken` is NOT cancelled by the monitor

#### Scenario: Queue execution starts regardless of past merge inactivity

**Given** the base branch has no merge commits within the configured stall threshold
**And** the user initiates parallel execution via `cflx queue` or TUI
**When** the parallel execution loop starts
**Then** queued changes are dispatched normally
**And** the merge stall monitor may emit a warning but does not prevent execution
