## ADDED Requirements

### Requirement: run Surfaces Hook Output

The `run` subcommand SHALL display hook execution details in the same user-visible CLI log stream used for other run progress messages.

#### Scenario: Hook command is logged before hook output

- **GIVEN** a hook is configured for a lifecycle stage reached during `cflx run`
- **WHEN** the hook starts
- **THEN** the CLI log first shows the hook type and expanded command string
- **AND** any captured hook output is displayed after the command log entry

#### Scenario: Hook output ordering includes failure result

- **GIVEN** a hook produces captured output and then fails during `cflx run`
- **WHEN** the run subcommand reports the failure
- **THEN** the CLI log shows hook command information first
- **AND** the CLI log shows captured hook output next
- **AND** the CLI log shows the hook failure result after the captured output

#### Scenario: CLI run preserves hook visibility parity with non-interactive execution

- **GIVEN** the same hook configuration is used in serial CLI run and another existing execution path that already emits hook logs
- **WHEN** the hook executes in serial CLI run
- **THEN** users can see the hook command and any captured output without needing debug-only tracing configuration
