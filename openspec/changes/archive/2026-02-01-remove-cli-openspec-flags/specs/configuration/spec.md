## ADDED Requirements
### Requirement: Configuration File is the Only Command Customization Method

OpenSpec command execution MUST NOT be overridden by CLI flags or environment variables.

The orchestrator SHALL define OpenSpec and agent execution methods through configuration file command templates only.

#### Scenario: CLI flag command override is disabled

- **WHEN** user runs `cflx --help`
- **THEN** --openspec-cmd is not listed
- **AND** CLI does not allow OpenSpec command override

#### Scenario: Environment variable command override is disabled

- **WHEN** OPENSPEC_CMD environment variable is set
- **THEN** CLI does not read or use this environment variable
- **AND** configuration file settings are used instead
