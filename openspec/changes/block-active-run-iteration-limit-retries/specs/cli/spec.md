## MODIFIED Requirements

### Requirement: Error State Display

When an error occurs, TUI SHALL explicitly display the error state and the authoritative retry eligibility for the failed change. Retry guidance SHALL be shown only when the shared service currently permits retry. While typed Apply iteration-limit evidence is owned by the active run, the TUI SHALL retain the diagnostic and display a stable active-limit explanation without promising Space or F5 retry.

#### Scenario: Mode transition on error

- **WHEN** opencode execution fails with an error (LLM error, insufficient credits, etc.)
- **THEN** TUI mode transitions to "Error"
- **AND** header status displays "Error" in red

#### Scenario: Status panel error display

- **WHEN** TUI is in error state
- **AND** the failed change is retry-eligible
- **THEN** status panel displays "Error in <change_id>"
- **AND** "Press F5 to retry" guidance is displayed

#### Scenario: Change display in error state

- **WHEN** TUI is in error state
- **THEN** the errored Change status shows "[error]" in red
- **AND** other queued Changes maintain their state

#### Scenario: Active iteration limit replaces retry guidance

- **WHEN** TUI displays an error change carrying typed Apply iteration-limit evidence owned by the active run
- **THEN** its retained error detail remains inspectable
- **AND** Space/F5 retry guidance is not displayed for that change
- **AND** the TUI explains that retry is unavailable until the active run closes

### Requirement: Error Retry with F5 Key

In error state, pressing F5 SHALL retry processing of the failed Change only when shared authoritative eligibility permits retry. If typed Apply iteration-limit evidence is still owned by the active run, F5 SHALL be a mutation-free refusal and SHALL NOT transition the TUI to Running. Once the owning run closes and eligibility is refreshed, F5 MAY start a later boundary through the ordinary retry path.

#### Scenario: Retry with F5 key

- **WHEN** TUI is in error state
- **AND** the errored Change is retry-eligible
- **AND** user presses F5 key
- **THEN** the errored Change is added back to the queue
- **AND** TUI transitions to "Running" mode
- **AND** processing resumes

#### Scenario: Log display on retry

- **WHEN** user initiates an eligible retry with F5 key
- **THEN** log panel displays "Retrying: <change_id>"

#### Scenario: State after successful retry

- **WHEN** the retried processing succeeds
- **THEN** the Change status updates to "completed" or "archived"
- **AND** remaining queued Changes continue processing

#### Scenario: F5 cannot target an active limited run

- **WHEN** TUI is in error state for a change carrying active-run Apply iteration-limit evidence
- **AND** user presses F5 key
- **THEN** no retry, queue, mark, explicit-retry, or scheduler command is emitted
- **AND** TUI remains out of Running mode
- **AND** the active-limit explanation remains visible

#### Scenario: F5 becomes available after boundary closure

- **GIVEN** the prior run's finish-hook ownership completed and its active limit gate was retired
- **WHEN** TUI refreshes authoritative eligibility for the still-retryable change
- **AND** user presses F5
- **THEN** ordinary retry admission may start a later scheduler boundary
- **AND** the later boundary uses workspace-derived state and a fresh Apply budget

### Requirement: Footer Dynamic Guidance Display

The selection and error-mode footer SHALL display guidance based on application state and authoritative per-change action eligibility. It SHALL NOT instruct the operator to use Space or F5 for a row whose active-run Apply iteration limit blocks retry.

#### Scenario: Guidance when no changes

- **WHEN** TUI is in selection mode
- **AND** the change list is empty
- **THEN** the footer displays "Add new proposals to get started"

#### Scenario: Guidance when no changes selected

- **WHEN** TUI is in selection mode
- **AND** one or more mark-eligible changes exist
- **AND** no changes are selected
- **THEN** the footer displays "Select changes with Space to process"

#### Scenario: Guidance when changes are selected

- **WHEN** TUI is in selection mode
- **AND** one or more eligible changes are selected
- **THEN** the footer displays "Press F5/! to start processing" when no TUI config override exists

#### Scenario: Limited error rows do not produce retry promises

- **WHEN** every otherwise visible error row is blocked by active-run Apply iteration-limit evidence
- **THEN** the footer does not instruct the operator to mark those rows with Space or retry them with F5
- **AND** it reports the stable active-limit condition

#### Scenario: Bulk mark selection excludes a limited error row

- **GIVEN** an active-run-limited error row and unrelated eligible rows are visible
- **WHEN** the operator invokes bulk execution-mark selection
- **THEN** the limited row's mark and Running queue intent remain unchanged
- **AND** eligible rows still receive the coherent bulk target state atomically
- **AND** the exclusion is explained with `apply_iteration_limit_active`
