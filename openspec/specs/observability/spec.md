# Observability Specification

## Purpose

This specification defines the logging and observability requirements for the Conflux orchestrator. It ensures that all command executions, TUI events, and system operations are properly logged for debugging and troubleshooting purposes.

The specification covers:
- Command execution logging (VCS, AI agents, hooks)
- TUI log synchronization to debug files
- Log level classification and formatting standards

## Requirements

### Requirement: REQ-OBS-001 Command Execution Logging

The orchestrator MUST log command information before executing external commands (`tokio::process::Command`, `std::process::Command`).

The log MUST include the following information:
- Executable name
- Argument list
- Working directory (if set)

**Addition**: All log entries displayed in the TUI Logs View MUST also be output to the debug log file when the `--logs` option is specified.

#### Scenario: VCS Command Execution Logging

- **GIVEN** creating a git worktree
- **WHEN** the `git worktree add` command is executed
- **THEN** the entire command line is logged at `debug!` level
- **AND** the working directory is included in the log

#### Scenario: AI Agent Command Execution Logging

- **GIVEN** applying a change
- **WHEN** an OpenCode agent command is executed
- **THEN** the command line is logged at `info!` level

#### Scenario: Hook Execution Logging

- **GIVEN** an on_apply_start hook is configured
- **WHEN** the hook command is executed
- **THEN** the command line is logged at `info!` level
- **AND** the log includes "Running on_apply_start hook" context

#### Scenario: TUI Logs Sync to Debug File

- **GIVEN** the TUI is started with `--logs /tmp/debug.log` option
- **WHEN** an error occurs during agent processing and is displayed in TUI Logs View
- **THEN** the same error message is also recorded in the debug log file at `ERROR` level
- **AND** the log includes the `tui_log` target

#### Scenario: Warning Log Sync

- **GIVEN** the TUI is started with `--logs /tmp/debug.log` option
- **WHEN** a merge is deferred and a warning is displayed in TUI Logs View
- **THEN** the same warning message is also recorded in the debug log file at `WARN` level

#### Scenario: Info Log Sync

- **GIVEN** the TUI is started with `--logs /tmp/debug.log` option
- **WHEN** processing starts and an info log is displayed in TUI Logs View
- **THEN** the same message is also recorded in the debug log file at `INFO` level

### Requirement: REQ-OBS-002 Appropriate Log Level Classification

The orchestrator MUST use appropriate log levels based on command importance.

Log level criteria:
- `info!`: Major user-facing operations (apply, archive, analyze, hook execution)
- `debug!`: Internal VCS commands, auxiliary command execution

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

### Requirement: REQ-OBS-003 Unified Log Format

The orchestrator MUST use a consistent log format.

Format rules:
- Before command execution: `"Running {context}: {command}"` or `"Executing {command}"`
- Include context information whenever possible (e.g., change ID, workspace path)

#### Scenario: Unified Format Log Output

- **GIVEN** multiple types of commands are executed
- **WHEN** checking the logs
- **THEN** all command logs are output in a unified format
- **AND** context information (change ID, etc.) is included

#### Scenario: Handling Long Command Lines

- **GIVEN** executing a command with very long arguments
- **WHEN** checking the logs
- **THEN** the entire command line is recorded (not truncated)
