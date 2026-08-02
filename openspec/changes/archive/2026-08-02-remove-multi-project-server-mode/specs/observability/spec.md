## MODIFIED Requirements

### Requirement: Startup logs include cflx version identity

When Conflux starts a retained user-facing runtime mode, the startup log MUST include enough version identity to determine which cflx binary produced the log.

The startup log MUST include at least the product name, `CARGO_PKG_VERSION`, and `BUILD_NUMBER`.

#### Scenario: Headless run startup log includes version identity
- **GIVEN** a user starts `cflx run`
- **WHEN** the process emits its startup `info!` log before orchestration begins
- **THEN** at least one startup log entry includes the cflx version and build number
- **AND** the log entry is persisted to the configured log file

### Requirement: Startup logs identify the runtime mode

Startup logs for retained user-facing runtime modes MUST identify whether the process started in TUI or run mode.

#### Scenario: Startup log distinguishes retained mode
- **GIVEN** a user starts either TUI or run mode
- **WHEN** the initial startup log is emitted
- **THEN** the log includes the runtime mode in human-readable form
- **AND** the mode information is visible without requiring correlation with separate events
