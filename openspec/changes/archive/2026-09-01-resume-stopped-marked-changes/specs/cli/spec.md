## MODIFIED Requirements

### Requirement: Interrupted Change Handling
Changes interrupted by stop SHALL hold queued status only during execution. Force-stop SHALL reset queue intent to NotQueued while preserving execution marks. A later explicit Start/F5 in process mode Stopped SHALL restore eligible execution-marked ordinary changes to queued, clear only the operator-Stopped terminal classification and stop-produced dequeue/runtime residue that prevents ordinary admission, and start one fresh scheduler boundary so they can be reprocessed without restarting the owner. Mark mutation, bulk mark, re-mark, refresh, or delayed mark settlement alone SHALL NOT resume stopped changes. Accepting status SHALL remain an in-flight execution state and MUST reset to NotQueued when force-stopped.

#### Scenario: Force-stopped accepting change returns to not queued
- **GIVEN** a change is in Accepting status
- **WHEN** the user force stops with second Esc press
- **THEN** the change status becomes not queued
- **AND** the execution mark remains set


#### Scenario: Force-stopped marked change resumes explicitly

- **GIVEN** a force-stopped ordinary change retains its execution mark and has NotQueued intent
- **WHEN** the user presses F5 in Stopped mode
- **THEN** the change becomes ordinary queued work without owner restart
- **AND** exactly one fresh scheduler boundary begins
- **AND** dependency analysis runs again

#### Scenario: Force-stopped mark mutation is not resume

- **GIVEN** a force-stopped change retains its execution mark
- **WHEN** the mark is set again or its settlement timer expires
- **THEN** the change remains stopped and NotQueued
- **AND** no scheduler or dependency analysis begins

<!-- Expected canonical result after archive: Interrupted Change Handling distinguishes preserved selection from explicit resume and no longer requires owner restart. -->
