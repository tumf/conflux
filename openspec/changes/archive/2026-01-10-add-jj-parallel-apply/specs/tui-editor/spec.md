## ADDED Requirements

### Requirement: Parallel Mode Toggle Key

The TUI SHALL support toggling parallel mode using the `=` key, but only when jj is available.

#### Scenario: Toggle parallel mode on with `=` key
- **GIVEN** TUI is in selection mode
- **AND** a `.jj` directory exists (jj repository detected)
- **AND** parallel mode is currently OFF
- **WHEN** user presses `=` key
- **THEN** parallel mode is enabled
- **AND** log displays "Parallel mode: ON"
- **AND** visual indicator shows parallel mode is active

#### Scenario: Toggle parallel mode off with `=` key
- **GIVEN** TUI is in selection mode
- **AND** parallel mode is currently ON
- **WHEN** user presses `=` key
- **THEN** parallel mode is disabled
- **AND** log displays "Parallel mode: OFF"
- **AND** visual indicator is removed

#### Scenario: `=` key hidden when jj not available
- **GIVEN** TUI is in selection mode
- **AND** no `.jj` directory exists
- **WHEN** TUI renders the footer help text
- **THEN** the `=: parallel` option is NOT displayed in help text
- **AND** pressing `=` key has no effect

#### Scenario: `=` key shown when jj available
- **GIVEN** TUI is in selection mode
- **AND** a `.jj` directory exists
- **WHEN** TUI renders the footer help text
- **THEN** the `=: parallel` option IS displayed in help text

### Requirement: Parallel Mode State Indicator

The TUI SHALL display a visual indicator when parallel mode is enabled.

#### Scenario: Parallel mode indicator in header
- **GIVEN** parallel mode is enabled
- **WHEN** TUI renders the header
- **THEN** a "[parallel]" badge is displayed in the header
- **AND** the badge uses a distinct color (e.g., cyan)

#### Scenario: No indicator when parallel mode off
- **GIVEN** parallel mode is disabled
- **WHEN** TUI renders the header
- **THEN** no parallel mode badge is displayed

### Requirement: Parallel Mode Toggle During Modes

The TUI SHALL restrict parallel mode toggling based on current app mode.

#### Scenario: Toggle allowed in selection mode
- **GIVEN** TUI is in Selecting mode
- **WHEN** user presses `=` key
- **THEN** parallel mode is toggled

#### Scenario: Toggle allowed in stopped mode
- **GIVEN** TUI is in Stopped mode
- **WHEN** user presses `=` key
- **THEN** parallel mode is toggled

#### Scenario: Toggle blocked in running mode
- **GIVEN** TUI is in Running mode
- **WHEN** user presses `=` key
- **THEN** parallel mode is NOT toggled
- **AND** log displays "Cannot toggle parallel mode while running"

#### Scenario: Toggle blocked in stopping mode
- **GIVEN** TUI is in Stopping mode
- **WHEN** user presses `=` key
- **THEN** parallel mode is NOT toggled

### Requirement: jj Detection at TUI Startup

The TUI SHALL detect jj availability at startup and cache the result.

#### Scenario: jj detected at startup
- **GIVEN** user starts the TUI
- **AND** a `.jj` directory exists in the current working directory
- **THEN** jj_available flag is set to true
- **AND** parallel mode features are enabled

#### Scenario: jj not detected at startup
- **GIVEN** user starts the TUI
- **AND** no `.jj` directory exists in the current working directory
- **THEN** jj_available flag is set to false
- **AND** parallel mode features are hidden
- **AND** no error is displayed (silent degradation)
