## MODIFIED Requirements
### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display dynamic key hints in running mode consistent with select mode.

Changes panel title SHALL show only change-related keys.
App-level control keys (Esc, Ctrl+C) SHALL be shown in Status panel title instead of Changes panel.

#### Scenario: Running mode shows appropriate keys
- **GIVEN** the TUI is in running mode
- **WHEN** changes exist
- **THEN** the Changes panel key hints SHALL show selection keys based on current item state
- **AND** the Changes panel title SHALL NOT show "Esc: stop"
- **AND** the Changes panel title SHALL NOT show "q: quit"

#### Scenario: Running mode with empty list
- **GIVEN** the TUI is in running mode
- **WHEN** the changes list is empty
- **THEN** the Changes panel key hints SHALL NOT show selection keys
- **AND** the Changes panel title SHALL NOT show "Esc: stop"
- **AND** the Changes panel title SHALL NOT show "q: quit"
