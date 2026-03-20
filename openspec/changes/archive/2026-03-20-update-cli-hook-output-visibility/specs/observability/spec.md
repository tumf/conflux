## ADDED Requirements

### Requirement: CLI Hook Logs Include Captured Streams

The orchestrator SHALL expose captured hook output to both user-visible CLI(run) logs and persistent observability sinks when that output is available.

#### Scenario: CLI hook stdout is observable without debug logging

- **GIVEN** `RUST_LOG` is unset and a configured hook writes to stdout during `cflx run`
- **WHEN** the hook output is captured
- **THEN** the output is visible in the normal CLI run log stream
- **AND** the same hook execution remains available to the configured persistent log sink

#### Scenario: CLI hook stderr includes stream context

- **GIVEN** a configured hook writes to stderr during `cflx run`
- **WHEN** the output is logged
- **THEN** the log message identifies that the content came from captured hook output
- **AND** the message preserves enough context to distinguish stderr-derived diagnostics from the hook command log itself

#### Scenario: Truncated hook output is not silent

- **GIVEN** captured hook output exceeds the configured display threshold
- **WHEN** the orchestrator emits the CLI-visible hook log
- **THEN** the emitted message states that truncation occurred
- **AND** the persistent log representation follows the same truncation signaling rule when truncation is applied there
