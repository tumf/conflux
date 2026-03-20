## MODIFIED Requirements

### Requirement: run Subcommand

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.

#### Scenario: Run with specific change

- **WHEN** user runs `cflx run --change <id>`
- **THEN** only the specified change is processed
- **AND** the snapshot log shows only the specified change

#### Scenario: Run with comma-separated changes

- **WHEN** user runs `cflx run --change a,b,c`
- **THEN** only changes `a`, `b`, `c` are processed
- **AND** the snapshot log shows only `a`, `b`, `c`

#### Scenario: Run with non-existent change

- **WHEN** user runs `cflx run --change nonexistent`
- **AND** no change named `nonexistent` exists
- **THEN** a warning message "Specified change 'nonexistent' not found, skipping" is displayed
- **AND** exits with "No changes found"

#### Scenario: Run with mixed valid and invalid changes

- **WHEN** user runs `cflx run --change a,nonexistent,c`
- **AND** `a` and `c` exist but `nonexistent` does not
- **THEN** a warning message "Specified change 'nonexistent' not found, skipping" is displayed
- **AND** only `a` and `c` are processed
- **AND** the snapshot log shows only `a`, `c`

#### Scenario: Successful run exits promptly

- **GIVEN** orchestration completes successfully and no restart was explicitly requested
- **WHEN** `cflx run` logs successful completion
- **THEN** the command exits promptly with status code 0
- **AND** it does not wait for an additional stop signal before terminating
