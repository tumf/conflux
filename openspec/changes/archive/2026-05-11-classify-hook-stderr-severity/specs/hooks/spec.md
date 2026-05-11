## MODIFIED Requirements

### Requirement: CLI Hook Output Visibility

The orchestrator SHALL surface hook command execution and captured hook output in normal CLI (`cflx run`) user-visible logs for every configured hook type.

Captured hook output severity SHALL reflect hook outcome. Stderr from a hook command that exits successfully SHALL remain visible as informational hook output and SHALL NOT be classified as warning/failure solely because the stream is stderr. Stderr from a hook command that fails SHALL remain visible as warning/error context before the failure is reported.

Hook output visibility is observational only and MUST NOT introduce hidden out-of-worktree durable workflow-control state.

#### Scenario: CLI run shows stdout from change hook

- **GIVEN** `hooks.pre_apply` is set to `echo 'hello from hook'`
- **AND** `cflx run` processes a change that executes `pre_apply`
- **WHEN** the hook completes
- **THEN** the CLI log shows the executed hook command
- **AND** the CLI log shows `hello from hook`

#### Scenario: successful hook stderr remains informational

- **GIVEN** `hooks.pre_apply` is set to `sh -c "echo 'hook diagnostic' 1>&2"`
- **AND** `cflx run` processes a change that executes `pre_apply`
- **WHEN** the hook exits zero
- **THEN** the captured stderr output remains visible
- **AND** the output is not emitted as a warning-level hook failure diagnostic solely because it came from stderr

#### Scenario: Hook failure still emits captured output

- **GIVEN** `hooks.post_apply` writes stderr output and then exits non-zero
- **AND** `continue_on_failure` is `false`
- **WHEN** the hook fails during `cflx run`
- **THEN** the captured stderr output is shown in warning/error context before the failure is reported
- **AND** the failure result still terminates or propagates according to hook configuration

#### Scenario: Truncated CLI hook output is marked explicitly

- **GIVEN** a configured hook writes output longer than the CLI display limit
- **WHEN** `cflx run` logs the captured hook output
- **THEN** the CLI log includes the visible prefix of the output
- **AND** the CLI log explicitly indicates that the output was truncated
