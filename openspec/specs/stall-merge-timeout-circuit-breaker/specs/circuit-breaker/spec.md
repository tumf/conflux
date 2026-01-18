## ADDED Requirements
### Requirement: Merge Stall Circuit Breaker

The orchestrator MUST detect a stall when merge progress to the base branch does not occur for a certain period and immediately stop all processing, including running operations.

#### Scenario: Immediate stop when no merge progress for 30 minutes
- **GIVEN** orchestration is running
- **AND** no `Merge change: <change_id>` merge commit has been added to the base branch for 30+ minutes
- **WHEN** a monitoring check is executed
- **THEN** a stall is detected
- **AND** all processing, including running operations, stops immediately
- **AND** the merge stall reason is logged as the stop reason

#### Scenario: Continue when merge progress occurs within monitoring period
- **GIVEN** orchestration is running
- **AND** a `Merge change: <change_id>` merge commit has been added to the base branch within the monitoring period
- **WHEN** a monitoring check is executed
- **THEN** no stall detection occurs and processing continues

#### Scenario: Detect merge stall and stop in parallel mode
- **GIVEN** the orchestrator is running in parallel mode
- **AND** merges from worktree to base branch create `Merge change: <change_id>` commits
- **WHEN** merge stall is detected
- **THEN** stop immediately

#### Scenario: No merge stall monitoring in serial mode
- **GIVEN** the orchestrator is running in serial mode
- **AND** serial mode does not create `Merge change: <change_id>` commits
- **WHEN** execution continues
- **THEN** merge stall monitoring is not executed, and only other circuit breakers (such as error detection) are applied

## MODIFIED Requirements
### Requirement: Identical Error Detection

The orchestrator SHALL detect consecutive identical errors and prevent infinite loops.

#### Scenario: Skip change when the same error occurs 5 times consecutively
- **GIVEN** a change has been applied 5 times consecutively
- **AND** each apply execution produces the same error message
- **WHEN** the orchestrator attempts the 6th apply
- **THEN** identical error detection is triggered and an error log is output
- **AND** that change is skipped and the system moves to the next

#### Scenario: Determine identity via error message normalization
- **GIVEN** the 1st error is "File not found: /path/to/file1"
- **AND** the 2nd error is "File not found: /path/to/file2"
- **WHEN** error messages are normalized and compared
- **THEN** the path portion is excluded and recognized as "File not found" pattern
- **AND** counted as identical errors

#### Scenario: JSON field names do not cause false positives
- **GIVEN** agent output includes a JSON field `"is_error": false`
- **WHEN** error detection processing is executed
- **THEN** JSON field names are excluded
- **AND** not falsely detected as errors

#### Scenario: No detection when different errors are mixed
- **GIVEN** the 1st error is "File not found"
- **AND** the 2nd error is "Permission denied"
- **AND** the 3rd error is "File not found"
- **WHEN** identical error detection is executed
- **THEN** not detected as consecutive
- **AND** processing continues normally

#### Scenario: Error detection threshold can be changed via config
- **GIVEN** config includes `error_circuit_breaker.threshold = 3`
- **WHEN** the same error occurs 3 times consecutively
- **THEN** identical error detection is triggered on the 3rd occurrence
- **AND** the change is skipped
