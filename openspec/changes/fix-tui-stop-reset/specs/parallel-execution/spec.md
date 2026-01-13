## MODIFIED Requirements
### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination (cancellation, graceful stop, or normal completion) using local state flags.

The system SHALL use this information to conditionally send completion events and messages.

The system SHALL reset stop/cancel flags at the start of a new parallel execution run so that previous runs do not affect the next run.

**Priority**: HIGH  
**Rationale**: The orchestrator needs to know why the processing loop ended to send appropriate events and messages.

#### Scenario: Tracking stopped or cancelled state

**Given** the parallel orchestration loop is running  
**When** the loop checks for cancellation or graceful stop  
**And** either condition is true  
**Then** a `stopped_or_cancelled` flag should be set to true  
**And** the loop should break  
**And** this flag should prevent sending completion events after the loop

#### Scenario: Tracking error state during batch processing

**Given** the parallel orchestration loop is processing batches  
**When** a batch execution returns an error  
**Then** a `had_errors` flag should be set to true  
**And** processing should continue with remaining batches  
**And** this flag should affect the final completion message when all batches finish

#### Scenario: Reset stop flags for a new run

**Given** a previous parallel execution ended due to graceful stop or cancellation  
**When** a new parallel execution run starts  
**Then** `stopped_or_cancelled` is reset to false before processing begins  
**And** completion events are evaluated only for the new run
