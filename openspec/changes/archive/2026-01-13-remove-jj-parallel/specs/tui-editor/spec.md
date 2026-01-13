## RENAMED Requirements
- FROM: `### Requirement: jj Detection at TUI Startup`
- TO: `### Requirement: Git Detection at TUI Startup`

## MODIFIED Requirements
### Requirement: Parallel Mode Toggle Key

The TUI SHALL support toggling parallel mode using the `=` key, but only when git is available.

#### Scenario: Toggle parallel mode on with `=` key
- **GIVEN** TUI is in selection mode
- **AND** a `.git` directory exists (git repository detected)
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

#### Scenario: `=` key hidden when git not available
- **GIVEN** TUI is in selection mode
- **AND** no `.git` directory exists
- **WHEN** TUI renders the footer help text
- **THEN** the `=: parallel` option is NOT displayed in help text
- **AND** pressing `=` key has no effect

#### Scenario: `=` key shown when git available
- **GIVEN** TUI is in selection mode
- **AND** a `.git` directory exists
- **WHEN** TUI renders the footer help text
- **THEN** the `=: parallel` option IS displayed in help text

### Requirement: Git Detection at TUI Startup

The TUI SHALL detect git availability at startup and cache the result.

#### Scenario: git detected at startup
- **GIVEN** user starts the TUI
- **AND** a `.git` directory exists in the current working directory
- **THEN** git_available flag is set to true
- **AND** parallel mode features are enabled

#### Scenario: git not detected at startup
- **GIVEN** user starts the TUI
- **AND** no `.git` directory exists in the current working directory
- **THEN** git_available flag is set to false
- **AND** parallel mode features are hidden
- **AND** no error is displayed (silent degradation)
