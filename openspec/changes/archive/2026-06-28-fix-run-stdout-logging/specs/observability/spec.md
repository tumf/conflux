## MODIFIED Requirements

### Requirement: REQ-OBS-002 Appropriate Log Level Classification

The orchestrator MUST use appropriate log levels based on command importance.

Log level criteria:
- `info!`: Major user-facing operations (apply, archive, analyze, hook execution)
- `debug!`: Internal VCS commands, auxiliary command execution
- Agent subprocess stderr: `info!` (agent CLIs such as opencode use stderr for normal operation output)
- Internal orchestrator warnings: `warn!`

The `OutputHandler` trait MUST distinguish between agent subprocess stderr (normal operation output) and internal orchestrator warnings. Agent subprocess stderr MUST be logged at `info` level via a dedicated `on_agent_stderr` method. The existing `on_stderr` method MUST remain at `warn` level for internal warnings.

Default terminal-facing `cflx run` logging MUST suppress `debug!` and `trace!` records while preserving `info!`, `warn!`, and `error!` records. Persistent file logging MAY retain more detailed diagnostic records and MUST NOT be reduced by the default stdout filter.

#### Scenario: Output Control with Default Log Level

- **GIVEN** RUST_LOG environment variable is not set (default)
- **WHEN** running the orchestrator
- **THEN** `info!` level command logs are displayed
- **AND** `debug!` level VCS command logs are not displayed

#### Scenario: Detailed Log Output in Debug Mode

- **GIVEN** RUST_LOG=debug is set
- **WHEN** running the orchestrator
- **THEN** all VCS command logs are displayed
- **AND** internal auxiliary command logs are also displayed

#### Scenario: Agent subprocess stderr is logged at info level

- **GIVEN** an AI agent command (e.g., opencode) writes progress output to stderr
- **WHEN** the orchestrator captures the stderr output
- **THEN** the output is logged at `info` level via `on_agent_stderr`
- **AND** the output is NOT logged at `warn` level

#### Scenario: Internal orchestrator warnings remain at warn level

- **GIVEN** the orchestrator generates an internal warning (e.g., hook failure, cancellation)
- **WHEN** the warning is recorded
- **THEN** the warning is logged at `warn` level via `on_warn` or `on_stderr`
- **AND** the warning is NOT logged at `info` level

#### Scenario: Run stdout suppresses internal diagnostics by default

- **GIVEN** `cflx run` initializes stdout logging for a normal non-interactive run
- **AND** internal code emits `debug!` VCS command logs or dependency `trace!` poller logs
- **WHEN** the run writes terminal-facing stdout logs
- **THEN** stdout does not include `DEBUG Executing git command`
- **AND** stdout does not include `TRACE registering event source with poller`
- **AND** stdout does not include `TRACE deregistering event source from poller`
- **AND** user-facing `INFO`, `WARN`, and `ERROR` logs remain eligible for stdout display

#### Scenario: Persistent logs retain diagnostic detail

- **GIVEN** `cflx run` initializes both stdout and persistent file logging
- **AND** internal code emits `debug!` diagnostic records
- **WHEN** the logging system records events
- **THEN** the stdout filter does not reduce the persistent file logging layer's configured diagnostic level
- **AND** operators can still inspect detailed diagnostics through persistent log files and `cflx logs`
