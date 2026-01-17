## ADDED Requirements
### Requirement: Deprecated Flags Removal

The CLI SHALL NOT accept --opencode-path or --openspec-cmd flags.

Configuration-based command templates SHALL be the only way to customize OpenSpec and agent execution.

#### Scenario: Deprecated flags are not available

- **WHEN** user runs `cflx --help`
- **THEN** --opencode-path and --openspec-cmd are not listed
- **AND** no environment variable OPENSPEC_CMD is supported

#### Scenario: Configuration file is the only command customization method

- **WHEN** user wants to customize OpenSpec command execution
- **THEN** user must use configuration file settings
- **AND** CLI flags do not override configuration

### Requirement: Enhanced Help Output

The CLI SHALL display all subcommands and main options explicitly in `cflx --help`.

#### Scenario: Web monitoring flags are shown in help

- **WHEN** user runs `cflx --help`
- **THEN** --web, --web-port, --web-bind are listed for `run` and `tui` subcommands
- **AND** --parallel, --max-concurrent, --dry-run, --vcs are listed for `run` subcommand
