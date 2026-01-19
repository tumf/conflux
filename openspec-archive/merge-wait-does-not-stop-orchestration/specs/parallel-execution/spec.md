## MODIFIED Requirements
### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination (cancellation, graceful stop, or normal completion) using local state flags.

The system SHALL use this information to conditionally send completion events and messages.

In addition, the system SHALL distinguish `merge_wait` as a non-terminal state so the orchestration loop continues for other runnable changes and does not stop early.

The system SHALL NOT send completion events or success messages while any change remains in `merge_wait`.

#### Scenario: Tracking stopped or cancelled state
- **Given** the parallel orchestration loop is running
- **When** the loop checks for cancellation or graceful stop
- **And** either condition is true
- **Then** a `stopped_or_cancelled` flag should be set to true
- **And** the loop should break
- **And** this flag should prevent sending completion events after the loop

#### Scenario: Tracking error state during batch processing
- **Given** the parallel orchestration loop is processing batches
- **When** a batch execution returns an error
- **Then** a `had_errors` flag should be set to true
- **And** processing should continue with remaining batches
- **And** this flag should affect the final completion message when all batches finish

#### Scenario: MergeWait does not stop orchestration
- **Given** at least one change is in `MergeWait` during parallel execution
- **And** at least one other change remains queued and runnable
- **When** the orchestration loop evaluates whether to continue
- **Then** the loop continues processing runnable changes
- **And** `MergeWait` is not treated as a terminal completion reason

#### Scenario: MergeWait suppresses completion events
- **Given** at least one change is in `MergeWait`
- **When** all runnable queued changes have finished for the current pass
- **Then** the orchestrator does not send `AllCompleted`-equivalent success events
- **And** the orchestrator does not emit success completion messages
