## MODIFIED Requirements

### Requirement: HTTP Server Lifecycle

When web monitoring is enabled for a local TUI or `cflx run` process, the HTTP server SHALL start as a process-scoped single-instance monitoring and remote-control endpoint. The system SHALL NOT provide a standalone multi-project server daemon.

#### Scenario: Local web monitoring starts

**Given**: A user starts local TUI or run mode with `--web`
**When**: Startup validation succeeds
**Then**: A process-scoped HTTP listener starts for that local orchestration instance

#### Scenario: No standalone daemon lifecycle

**Given**: The installed CLI
**When**: A user requests CLI help
**Then**: No standalone multi-project server command or service lifecycle is advertised

### Requirement: Configuration Options

The HTTP monitoring server SHALL be configured only by retained `--web*` options and their local web configuration. Removed multi-project `server.*` configuration SHALL NOT be required to start or use local web monitoring.

#### Scenario: Retained web options configure listener

**Given**: A local TUI or run invocation
**When**: The user supplies retained web bind, port, token, token-env, or allowed-origin options
**Then**: Those values configure the process-scoped web listener under existing validation rules

#### Scenario: Server configuration is absent

**Given**: A configuration without obsolete `server.*` fields
**When**: Local web monitoring starts
**Then**: Startup does not require multi-project server configuration
