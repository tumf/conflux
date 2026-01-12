# Spec: Parallel Execution Completion Messages

## ADDED Requirements

### Requirement: Parallel execution completion status must accurately reflect actual processing outcome

The system SHALL send completion events and messages only when processing completes normally, not when stopped or cancelled by the user.

The system SHALL distinguish between successful completion, completion with errors, graceful stop, and cancellation.

**Priority**: HIGH  
**Rationale**: Incorrect completion messages mislead users about the processing status and can cause confusion when resuming work.

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

### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination (cancellation, graceful stop, or normal completion) using local state flags.

The system SHALL use this information to conditionally send completion events and messages.

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

## Implementation Notes

### Modified Files

- `src/tui/orchestrator.rs`: Main implementation file
  - Function: `run_orchestrator_parallel()`
  - Add state tracking flags before loop (line ~800)
  - Set flags in cancellation check (line ~809)
  - Set flags in graceful stop check (line ~820)
  - Set flags in error handling (line ~948)
  - Replace unconditional completion messages (line ~960-966) with conditional logic

### State Tracking

```rust
let mut stopped_or_cancelled = false;
let mut had_errors = false;
```

### Completion Logic

```rust
if !stopped_or_cancelled {
    if had_errors {
        // Send warning message for partial success
    } else {
        // Send success message for complete success
    }
    // Send AllCompleted event
}
// If stopped_or_cancelled is true, send nothing
```

## Testing Requirements

### Unit Tests

- Test loop termination flag logic independently
- Verify flag states for each termination path

### Integration Tests

- E2E test for graceful stop scenario
- E2E test for force cancel scenario
- E2E test for successful completion
- E2E test for completion with errors

### Manual Testing

- TUI verification for each scenario
- Log output verification
- State transition verification
