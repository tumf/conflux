## ADDED Requirements

### Requirement: CLI startup behavior emits versioned startup logs

The CLI SHALL emit a versioned startup log for each primary runtime entry path: default TUI launch, explicit `tui`, `run`, and `server`.

Versioned startup logs MUST use a consistent version/build representation so operators can compare startup logs across modes without interpreting different formats.

#### Scenario: Default TUI launch emits versioned startup log
- **GIVEN** a user runs `cflx` with no subcommand
- **WHEN** the CLI initializes logging and starts the TUI runtime
- **THEN** the startup log includes the cflx version and build number
- **AND** the startup log identifies the mode as TUI

#### Scenario: Explicit TUI launch emits versioned startup log
- **GIVEN** a user runs `cflx tui`
- **WHEN** the CLI initializes logging and starts the TUI runtime
- **THEN** the startup log includes the cflx version and build number
- **AND** the startup log identifies the mode as TUI

#### Scenario: Run subcommand emits versioned startup log
- **GIVEN** a user runs `cflx run`
- **WHEN** the CLI starts the orchestration runtime
- **THEN** the startup log includes the cflx version and build number
- **AND** the startup log identifies the mode as run

#### Scenario: Server subcommand emits versioned startup log
- **GIVEN** a user runs `cflx server`
- **WHEN** the CLI starts the server runtime
- **THEN** the startup log includes the cflx version and build number
- **AND** the startup log identifies the mode as server
