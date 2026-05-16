## ADDED Requirements

### Requirement: Logs Command

The Conflux CLI SHALL expose a `logs` subcommand for read-only access to persistent Conflux log files.

The command SHALL support printing the selected path, printing a bounded recent tail, following appended lines, preferring today's log file, and selecting a log project by explicit project slug.

#### Scenario: Logs command help documents modes

- **GIVEN** the Conflux binary is available
- **WHEN** the user runs `cflx logs --help`
- **THEN** the help output documents path printing
- **AND** the help output documents bounded tail output
- **AND** the help output documents follow mode
- **AND** the help output documents today's log preference
- **AND** the help output documents explicit project slug selection

#### Scenario: Logs command is read-only

- **GIVEN** persistent Conflux logs already exist
- **WHEN** the user runs `cflx logs --path` or `cflx logs --last 1`
- **THEN** the command reads or reports log locations only
- **AND** the command does not initialize the normal runtime file log sink for the purpose of viewing logs
