## ADDED Requirements

### Requirement: CLI Hook Output Visibility

The orchestrator SHALL surface hook command execution and captured hook output in normal CLI (`cflx run`) user-visible logs for every configured hook type.

#### Scenario: CLI run shows stdout from change hook

- **GIVEN** `hooks.pre_apply` is set to `echo 'hello from hook'`
- **AND** `cflx run` processes a change that executes `pre_apply`
- **WHEN** the hook completes
- **THEN** the CLI log shows the executed hook command
- **AND** the CLI log shows `hello from hook`

#### Scenario: CLI run shows stderr from change hook

- **GIVEN** `hooks.pre_apply` is set to `sh -c "echo 'hook warning' 1>&2"`
- **AND** `cflx run` processes a change that executes `pre_apply`
- **WHEN** the hook completes
- **THEN** the CLI log shows the executed hook command
- **AND** the CLI log shows the captured stderr output

#### Scenario: CLI run shows output from global hook without change id

- **GIVEN** `hooks.on_start` is set to `echo 'starting run'`
- **WHEN** `cflx run` starts orchestration
- **THEN** the CLI log shows the executed `on_start` hook command
- **AND** the CLI log shows `starting run`

#### Scenario: Hook failure still emits captured output

- **GIVEN** `hooks.post_apply` writes output and then exits non-zero
- **AND** `continue_on_failure` is `false`
- **WHEN** the hook fails during `cflx run`
- **THEN** any captured hook output is shown in the CLI log before the failure is reported
- **AND** the failure result still terminates or propagates according to hook configuration

#### Scenario: Truncated CLI hook output is marked explicitly

- **GIVEN** a configured hook writes output longer than the CLI display limit
- **WHEN** `cflx run` logs the captured hook output
- **THEN** the CLI log includes the visible prefix of the output
- **AND** the CLI log explicitly indicates that the output was truncated
