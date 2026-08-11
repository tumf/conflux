## MODIFIED Requirements

### Requirement: Error Retry with F5 Key

Pressing a configured Start key such as F5 SHALL retry marked retry-eligible change-local failures according to shared authoritative eligibility without requiring the process-wide execution mode to be Error. Running mode SHALL permit retry-only Start against a live scheduler. Select and Stopped SHALL preserve ordinary marked `not queued` Start priority and SHALL fall back to marked retry routes only when no ordinary target is startable. Process-wide Error SHALL retain its existing retry behavior, and Stopping SHALL refuse Start without mutation.

If typed Apply iteration-limit evidence is still owned by the active run, F5 SHALL be a mutation-free refusal and SHALL NOT transition the TUI to Running. Once the owning run closes and eligibility is refreshed, F5 MAY start a later boundary through the ordinary retry path. A typed runtime-limit failure SHALL NOT be retried automatically by the failed invocation's scheduler cycle, but it MAY be retried by a later explicit F5 request when shared eligibility permits it.

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
- **AND** `alpha` SHALL be excluded with target-specific status detail
- **AND** the request SHALL NOT implicitly retry `alpha`

#### Scenario: Later explicit retry follows runtime-limit termination

- **GIVEN** an Apply invocation for `alpha` was terminated by its absolute runtime limit
- **AND** that scheduler cycle did not retry the invocation automatically
- **AND** the resulting terminal Error evidence is now retry-eligible
- **WHEN** the operator marks `alpha` and presses F5
- **THEN** the new operator command MAY retry `alpha`
- **AND** the prior no-automatic-retry guarantee SHALL remain satisfied

#### Scenario: Active iteration limit remains mutation-free

- **GIVEN** marked error change `alpha` carries typed Apply iteration-limit evidence owned by a live run
- **WHEN** the user presses F5 in Select, Running, Stopped, or Error
- **THEN** Start SHALL refuse or exclude `alpha` according to the shared retry contract
- **AND** reducer status, error detail, mark, queue, explicit-retry edges, scheduler, and mode SHALL remain unchanged

<!-- Expected canonical result after archive: F5 retry is governed by marked target evidence and lifecycle safety rather than requiring process-wide Error mode, while ordinary Start priority and active-limit refusal remain intact. -->
