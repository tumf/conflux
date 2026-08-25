## MODIFIED Requirements

### Requirement: Persistent-idle Ready remains a live run-control target

When a typed persistent-scheduler idle transition projects Ready/`select` while the scheduler task remains alive, TUI and Web MUST use the process-local `persistent_scheduler_idle` fact to distinguish this state from pre-run Select and retain live-scheduler controls. A graceful stop requested from this state MUST settle through the shared lifecycle boundary. When no executable, queued, admitted, active, resolve, merge, or cleanup work remains, the process MUST reach inactive `Stopped`/Ready and MUST NOT retain `Stopping` while waiting for a nonexistent work boundary.

Cancel-stop MUST remain valid only while a genuine graceful stop is pending. It MUST restore Ready when the stop originated from a persistent-idle episode and neither accepted Start nor typed work-start evidence opened a later run episode. It MAY restore Running only when accepted Start or typed work-start evidence cleared the idle episode. Core, TUI, and Web MUST project the same result from the authoritative shared outcome.

#### Scenario: No-work graceful stop reaches inactive Ready

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler has no executable, queued, admitted, active, resolve, merge, or cleanup work
- **WHEN** graceful stop is accepted and the scheduler settles the request
- **THEN** Core, TUI, and Web leave `Stopping`
- **AND** the process reaches inactive `Stopped` whose TUI header is Ready
- **AND** no synthetic queue intent, work-start event, or mark mutation is introduced

#### Scenario: Cancel idle-origin stop does not invent Running

- **GIVEN** graceful stop originated from persistent-idle Ready
- **AND** no accepted Start or typed work-start event has opened a later run episode
- **WHEN** F5 submits cancel-stop before terminal stop settlement
- **THEN** Core, TUI, and Web return to Ready / `select`
- **AND** they do not claim Running merely because the stop was withdrawn

#### Scenario: Cancel stop after real work restores Running

- **GIVEN** accepted Start or typed work-start evidence cleared the persistent-idle episode
- **AND** graceful stop then projected Stopping while the run remains live
- **WHEN** cancel-stop is accepted
- **THEN** Core, TUI, and Web return to Running
- **AND** active-work graceful-stop semantics remain unchanged
