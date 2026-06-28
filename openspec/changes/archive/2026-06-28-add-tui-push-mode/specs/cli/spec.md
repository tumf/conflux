## MODIFIED Requirements

### Requirement: Subcommand Structure

CLI SHALL have a subcommand structure that supports future command extensions. When `--push [remote]` is provided on the top-level TUI entrypoint without an explicit subcommand, the CLI SHALL launch the local TUI with push post-archive mode configured for local parallel execution. If the remote argument is omitted, the remote SHALL default to `origin`. The remote argument MUST NOT contain `:`; branch selection is unsupported and MUST be rejected before TUI orchestration starts. Top-level TUI `--push` MUST NOT be accepted with `--server` until remote server control can explicitly carry push post-archive configuration.

<!-- Expected canonical result after archive: `Subcommand Structure` will document that bare `cflx --push [remote]` launches local TUI with parallel push post-archive mode and rejects unsupported branch-selection or remote-server combinations. -->

#### Scenario: Bare TUI push mode defaults to origin

- **WHEN** user runs `cflx --push`
- **THEN** the interactive TUI is launched in local mode
- **AND** TUI parallel execution is configured for push post-archive action
- **AND** the selected remote is `origin`

#### Scenario: Bare TUI push mode accepts remote name

- **WHEN** user runs `cflx --push upstream`
- **THEN** the interactive TUI is launched in local mode
- **AND** TUI parallel execution is configured for push post-archive action
- **AND** the selected remote is `upstream`

#### Scenario: Bare TUI push mode rejects branch selection

- **WHEN** user runs `cflx --push origin:main`
- **THEN** TUI orchestration does not start
- **AND** the CLI reports that branch selection is not supported for `--push`

#### Scenario: Bare TUI push mode rejects remote server mode

- **WHEN** user runs `cflx --push --server http://host:39876`
- **THEN** TUI orchestration does not start
- **AND** the CLI reports that `--push` is not supported with remote TUI server mode

### Requirement: run Subcommand

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop only when the operator provides an explicit target mode: `--all`, one or more positional change IDs, or the legacy `--change` option. Positional IDs and legacy `--change` values SHALL be normalized into the same explicit change ID target list. `--all` SHALL target all current changes from the initial run snapshot. The target modes SHALL be mutually exclusive.

When `--push [remote]` is provided with parallel execution, `run` SHALL use push post-archive mode instead of base-merge post-archive mode. If the remote argument is omitted, the remote SHALL default to `origin`. The remote argument MUST NOT contain `:`; branch selection is unsupported and MUST be rejected before orchestration starts.

The explicit `tui` subcommand SHALL accept `--push [remote]` with the same remote defaulting and validation rules as `run --push`. For local TUI mode, the option SHALL configure push post-archive mode for local parallel execution. The option MUST NOT be accepted with `tui --server` until remote server control can explicitly carry push post-archive configuration.

<!-- Expected canonical result after archive: `run Subcommand` will continue to define existing run target behavior and will additionally document that explicit `cflx tui --push [remote]` configures local TUI parallel push post-archive mode with the same remote parsing rules as `run --push`. -->

#### Scenario: Explicit TUI push mode defaults to origin

- **WHEN** user runs `cflx tui --push`
- **THEN** the interactive TUI is launched in local mode
- **AND** TUI parallel execution is configured for push post-archive action
- **AND** the selected remote is `origin`

#### Scenario: Explicit TUI push mode accepts remote name

- **WHEN** user runs `cflx tui --push upstream`
- **THEN** the interactive TUI is launched in local mode
- **AND** TUI parallel execution is configured for push post-archive action
- **AND** the selected remote is `upstream`

#### Scenario: Explicit TUI push mode rejects branch selection

- **WHEN** user runs `cflx tui --push origin:main`
- **THEN** TUI orchestration does not start
- **AND** the CLI reports that branch selection is not supported for `--push`

#### Scenario: Explicit TUI push mode rejects remote server mode

- **WHEN** user runs `cflx tui --push --server http://host:39876`
- **THEN** TUI orchestration does not start
- **AND** the CLI reports that `--push` is not supported with remote TUI server mode

### Requirement: Enhanced Help Output

The CLI SHALL provide comprehensive help output that includes all subcommands, key options, and usage examples. Help output SHALL include the `--push [remote]` option anywhere it is accepted: top-level TUI launch, `run`, and `tui`.

<!-- Expected canonical result after archive: `Enhanced Help Output` will include `--push [remote]` in top-level and TUI help expectations. -->

#### Scenario: Main help shows push option

- **WHEN** user runs `cflx --help`
- **THEN** help output includes `--push [remote]`
- **AND** help output indicates it configures push post-archive behavior for local TUI parallel execution

#### Scenario: TUI subcommand help shows push option

- **WHEN** user runs `cflx tui --help`
- **THEN** help output includes `--push [remote]`
- **AND** help output indicates it configures push post-archive behavior for local TUI parallel execution
