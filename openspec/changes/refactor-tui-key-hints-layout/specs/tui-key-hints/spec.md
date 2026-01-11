## MODIFIED Requirements

### Requirement: Context-Aware Key Hints in Select Mode

The TUI SHALL display only actionable key hints based on current state in selection mode.

Changes panel title SHALL show only change-related keys.
App-level control keys SHALL be shown in Footer/Status panel instead of Changes panel.

#### Scenario: Empty changes list hides selection keys

- **GIVEN** the TUI is in select mode
- **WHEN** the changes list is empty
- **THEN** the Changes panel key hints SHALL NOT show "Space: queue"
- **AND** the Changes panel key hints SHALL NOT show "@: approve"
- **AND** the Changes panel key hints SHALL NOT show "e: edit"
- **AND** the Changes panel key hints SHALL show "↑↓/jk: move"
- **AND** the Changes panel title SHALL NOT show "q: quit"
- **AND** the Footer panel SHALL show "q: quit"

#### Scenario: No queued changes hides F5 key

- **GIVEN** the TUI is in select mode
- **AND** changes exist but none are selected for queue
- **THEN** the Changes panel key hints SHALL NOT show "F5: run"
- **AND** the Changes panel key hints SHALL show selection keys (Space/@/e)
- **AND** the Changes panel title SHALL NOT show "q: quit"
- **AND** the Footer panel SHALL show "q: quit"

#### Scenario: Queued changes shows F5 key

- **GIVEN** the TUI is in select mode
- **AND** at least one change is selected for queue
- **THEN** the Changes panel key hints SHALL show "F5: run"
- **AND** the Changes panel title SHALL NOT show "q: quit"
- **AND** the Footer panel SHALL show "q: quit"

### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display dynamic key hints in running mode consistent with select mode.

Changes panel title SHALL show only change-related keys.
App-level control keys (Esc, q) SHALL be shown in Status panel instead of Changes panel.

#### Scenario: Running mode shows appropriate keys

- **GIVEN** the TUI is in running mode
- **WHEN** changes exist
- **THEN** the Changes panel key hints SHALL show selection keys based on current item state
- **AND** the Changes panel title SHALL NOT show "Esc: stop"
- **AND** the Changes panel title SHALL NOT show "q: quit"
- **AND** the Status panel SHALL show "Esc: stop"
- **AND** the Status panel SHALL show "q: quit"

#### Scenario: Running mode with empty list

- **GIVEN** the TUI is in running mode
- **WHEN** the changes list is empty
- **THEN** the Changes panel key hints SHALL NOT show selection keys
- **AND** the Changes panel title SHALL NOT show "Esc: stop"
- **AND** the Changes panel title SHALL NOT show "q: quit"
- **AND** the Status panel SHALL show "Esc: stop"
- **AND** the Status panel SHALL show "q: quit"

## ADDED Requirements

### Requirement: App Control Keys in Status Panel

The TUI SHALL display app-level control keys in the Status panel when in running/stopping/stopped modes.

#### Scenario: Status panel shows stop and quit keys in Running mode

- **GIVEN** the TUI is in running mode
- **THEN** the Status panel text SHALL include "Esc: stop"
- **AND** the Status panel text SHALL include "q: quit"

#### Scenario: Status panel shows force stop in Stopping mode

- **GIVEN** the TUI is in stopping mode
- **THEN** the Status panel text SHALL include "Esc: force stop"
- **AND** the Status panel text SHALL include "q: quit"

#### Scenario: Status panel shows resume key in Stopped mode

- **GIVEN** the TUI is in stopped mode
- **THEN** the Status panel text SHALL include "F5: resume"
- **AND** the Status panel text SHALL include "q: quit"

### Requirement: App Control Keys in Footer Panel

The TUI SHALL display app-level control keys in the Footer panel when in select mode without logs.

#### Scenario: Footer panel shows quit key in Select mode

- **GIVEN** the TUI is in select mode
- **AND** the Logs panel is not visible (no logs yet)
- **THEN** the Footer panel SHALL show "q: quit"
