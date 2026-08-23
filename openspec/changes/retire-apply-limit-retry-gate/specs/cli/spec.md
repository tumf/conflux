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

- **GIVEN** an Apply invocation for `alpha` terminated at its iteration limit
- **AND** that invocation did not retry automatically
- **AND** the resulting terminal Error retains the iteration-limit diagnostic
- **AND** the persistent scheduler remains live
- **WHEN** the operator marks `alpha` and presses F5
- **THEN** the new operator command SHALL retry `alpha` exactly once
- **AND** the new invocation SHALL receive fresh Apply budget
- **AND** the prior no-automatic-retry guarantee SHALL remain satisfied

<!-- Expected canonical result after archive: F5 retries settled Apply-limit errors through explicit intent even when the persistent scheduler remains live, without enabling automatic redispatch. -->
