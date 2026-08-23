## MODIFIED Requirements

### Requirement: Error Retry with F5 Key

Pressing a configured Start key such as F5 SHALL retry marked retry-eligible targets according to shared authoritative eligibility without requiring the process-wide execution mode to be Error. Running mode SHALL permit retry-only Start against a live scheduler. Select and Stopped SHALL preserve ordinary marked `not queued` Start priority and SHALL fall back to marked retry routes only when no ordinary target is startable. Process-wide Error SHALL retain its existing retry behavior, and Stopping SHALL refuse Start without mutation.

A settled terminal Error that retains typed Apply iteration-limit evidence SHALL be retryable by a later explicit F5 request, including while the persistent scheduler remains live. The failed invocation's scheduler cycle SHALL NOT retry the target automatically. The TUI SHALL keep the retained diagnostic visible while presenting the same retry eligibility enforced by the shared command service.

#### Scenario: Retry with F5 key

- **WHEN** a retry-eligible Change is execution-marked
- **AND** user presses F5 key in a mode that permits retry-class Start
- **THEN** the Change is admitted through explicit retry
- **AND** processing resumes through a live or newly started scheduler boundary as applicable

#### Scenario: Log display on retry

- **WHEN** user initiates an eligible retry with F5 key
- **THEN** log panel displays "Retrying: <change_id>"

#### Scenario: State after successful retry

- **WHEN** the retried processing succeeds
- **THEN** the Change status updates to "completed" or "archived"
- **AND** remaining queued Changes continue processing

#### Scenario: Persistent-idle Select retries a marked change-local error

- **GIVEN** a persistent scheduler is alive and presented as Select
- **AND** change `alpha` carries retry-eligible terminal Error evidence and is execution-marked
- **WHEN** the user presses F5
- **THEN** Start SHALL route `alpha` through explicit retry rather than ordinary `not queued` admission
- **AND** the live scheduler SHALL be notified without spawning a second scheduler
- **AND** dependency analysis SHALL resume without waiting for ordinary mark settlement

#### Scenario: Running retries a marked change-local error

- **GIVEN** the process remains Running after `alpha` emitted `ProcessingError`
- **AND** unrelated work remains active or queued
- **AND** `alpha` is re-marked and currently retry-eligible
- **WHEN** the user presses F5
- **THEN** `alpha` SHALL enter explicit retry through the shared Start transaction
- **AND** unrelated work SHALL remain unchanged
- **AND** the process SHALL NOT enter global Error solely to authorize the retry

#### Scenario: Ordinary Start retains priority over retry-only marks

- **GIVEN** Select or Stopped has marked ordinary `not queued` change `beta`
- **AND** marked retry-only change `alpha` is also present
- **WHEN** the user presses F5
- **THEN** ordinary Start SHALL admit `beta`
- **AND** `alpha` SHALL be excluded with target-specific status detail that explains ordinary marks must be removed before retry-class Start can select `alpha`
- **AND** the request SHALL NOT implicitly retry `alpha`

#### Scenario: Later explicit retry follows runtime-limit termination

- **GIVEN** an Apply invocation for `alpha` was terminated by its absolute runtime limit
- **AND** that scheduler cycle did not retry the invocation automatically
- **AND** the resulting terminal Error evidence is now retry-eligible
- **WHEN** the operator marks `alpha` and presses F5
- **THEN** the new operator command MAY retry `alpha`
- **AND** the prior no-automatic-retry guarantee SHALL remain satisfied

#### Scenario: Later explicit retry follows iteration-limit termination

- **GIVEN** an Apply invocation for `alpha` terminated at its iteration limit
- **AND** that invocation did not retry automatically
- **AND** the resulting terminal Error retains the iteration-limit diagnostic
- **AND** the persistent scheduler remains live
- **WHEN** the operator marks `alpha` and presses F5
- **THEN** the new operator command SHALL retry `alpha` exactly once
- **AND** the new invocation SHALL receive fresh Apply budget
- **AND** the prior no-automatic-retry guarantee SHALL remain satisfied

### Requirement: Error State Display

When an error occurs, TUI SHALL explicitly display the error state and the authoritative retry eligibility for the failed change. Retry guidance SHALL be shown only when the shared service currently permits retry. A retained Apply iteration-limit diagnostic SHALL remain inspectable as evidence, and for a settled terminal error it SHALL NOT suppress retry guidance that shared eligibility currently permits.

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

#### Scenario: Settled iteration limit keeps authoritative retry guidance

- **WHEN** TUI displays a settled terminal-error change retaining typed Apply iteration-limit evidence
- **AND** the shared service currently permits retry for that change
- **THEN** its retained error detail and attempts/max evidence remain inspectable
- **AND** Space/F5 retry guidance is displayed for that change

### Requirement: Footer Dynamic Guidance Display

The selection and error-mode footer SHALL display guidance based on application state and authoritative per-change action eligibility. It SHALL NOT instruct the operator to use Space or F5 for a row whose authoritative eligibility currently refuses retry, and it SHALL NOT withhold that guidance for a settled terminal-error row merely because a retained Apply iteration-limit diagnostic exists.

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

#### Scenario: Settled limited error rows receive ordinary retry guidance

- **WHEN** every otherwise visible error row is a settled terminal error retaining typed Apply iteration-limit evidence
- **AND** shared eligibility permits retry for those rows
- **THEN** the footer presents the same Space/F5 guidance as for ordinary retryable error rows
- **AND** no stable active-limit condition replaces that guidance

#### Scenario: Bulk mark selection includes a settled limited error row

- **GIVEN** a settled limited terminal-error row and unrelated eligible rows are visible
- **WHEN** the operator invokes bulk execution-mark selection
- **THEN** the settled row is classified exactly as an ordinary terminal-error row
- **AND** eligible rows still receive the coherent bulk target state atomically
- **AND** no `apply_iteration_limit_active` exclusion is reported

<!-- Expected canonical result after archive: F5 retries settled Apply-limit errors through explicit intent even when the persistent scheduler remains live, without enabling automatic redispatch; error display and footer guidance follow the same shared eligibility instead of an active-limit exception. -->
