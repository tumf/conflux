## MODIFIED Requirements

### Requirement: Subcommand Structure

CLI SHALL have a subcommand structure that supports future command extensions. Bare invocation SHALL launch local TUI. Bare local TUI SHALL accept the same opt-in cumulative upstream integration options as explicit local `tui`: value-less `-u` and `--integrate-upstream` select `origin`, a named remote is accepted only as `--integrate-upstream=<remote>`, and enablement requires `--upstream-verify-command <command>`.

When `--push [remote]` is provided on the top-level TUI entrypoint without an explicit subcommand, the CLI SHALL launch local TUI with push post-archive mode configured for local parallel execution. Upstream integration and push post-archive mode MUST remain mutually exclusive. Top-level upstream integration and `--push` MUST NOT be accepted with `--server` because remote-client TUI does not own the local cumulative base.

#### Scenario: run without subcommand

- **WHEN** user runs `cflx` without arguments
- **THEN** the interactive local TUI is launched
- **AND** the change list is displayed in selection mode
- **AND** upstream integration is disabled

#### Scenario: bare TUI enables upstream integration

- **WHEN** user runs `cflx -u --upstream-verify-command '<command>'`
- **THEN** the interactive TUI is launched in local cumulative parallel mode
- **AND** TUI orchestration receives the same upstream runtime configuration as `cflx run -u`
- **AND** selected remote is `origin`

#### Scenario: bare TUI accepts explicit upstream remote

- **WHEN** user runs `cflx --integrate-upstream=upstream --upstream-verify-command '<command>'`
- **THEN** local TUI upstream integration selects remote `upstream`
- **AND** the option does not configure push post-archive mode

#### Scenario: bare TUI rejects incompatible publication modes

- **WHEN** user combines upstream integration with `--push` or `--server`
- **THEN** TUI orchestration does not start
- **AND** the CLI identifies the incompatible options before repository mutation

#### Scenario: run with unknown subcommand

- **WHEN** user runs with a non-existent subcommand
- **THEN** an error message with available subcommands is displayed

### Requirement: run Subcommand

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop only when the operator provides an explicit target mode: `--all`, one or more positional change IDs, or the legacy `--change` option. Positional IDs and legacy `--change` values SHALL be normalized into the same explicit change ID target list. `--all` SHALL target all current changes from the initial run snapshot. The target modes SHALL be mutually exclusive.

The `run` subcommand, explicit local `tui` subcommand, and bare local TUI SHALL expose one normalized cumulative upstream integration contract. Value-less `-u` and `--integrate-upstream` SHALL select `origin`; a named remote SHALL require `--integrate-upstream=<remote>`; enablement SHALL require `--upstream-verify-command <command>`. This capability SHALL use cumulative base merge plus change-scoped upstream publication and SHALL NOT use push post-archive mode.

When `--push [remote]` is provided with parallel execution, `run` or local TUI SHALL instead use push post-archive mode. If the remote argument is omitted, the remote SHALL default to `origin`. Upstream integration and `--push` SHALL be mutually exclusive. Upstream integration SHALL be rejected for remote-client `tui --server`, server orchestration, or serial effective execution before work starts.

#### Scenario: run enables per-change upstream publication

- **WHEN** user runs `cflx run --all -u --upstream-verify-command '<command>'`
- **THEN** run uses cumulative base integration and change-scoped upstream publication for remote `origin`
- **AND** it does not configure push post-archive mode
- **AND** each successful targeted change requires terminal `pushed`

#### Scenario: explicit local TUI enables equivalent upstream publication

- **WHEN** user runs `cflx tui --integrate-upstream=upstream --upstream-verify-command '<command>'`
- **THEN** local TUI installs the same upstream publication runtime used by run
- **AND** selected remote is `upstream`
- **AND** each completed change publishes without waiting for TUI shutdown or scheduler drain

#### Scenario: missing verification command is rejected

- **WHEN** user supplies `-u` or `--integrate-upstream` to run or local TUI without a non-empty `--upstream-verify-command`
- **THEN** orchestration does not start
- **AND** no fetch, worktree, merge, verification, or push side effect occurs

#### Scenario: upstream integration rejects push post-archive mode

- **WHEN** user combines `-u` or `--integrate-upstream` with `--push`
- **THEN** parsing or startup validation fails before orchestration
- **AND** the diagnostic explains that cumulative upstream publication and change-branch push are distinct mutually exclusive modes

#### Scenario: explicit TUI rejects upstream remote server mode

- **WHEN** user runs `cflx tui -u --upstream-verify-command '<command>' --server http://host:39876`
- **THEN** TUI orchestration does not start
- **AND** the CLI reports that upstream integration is available only to local TUI

#### Scenario: successful opted-in run exits after all changes are pushed

- **GIVEN** every targeted successful change has completed remote-confirmed upstream publication
- **WHEN** `cflx run` reports successful completion
- **THEN** every such change has display status `pushed`
- **AND** the command exits promptly with status code 0
- **AND** it does not wait for an additional stop signal

#### Scenario: local TUI remains active after pushed change

- **GIVEN** local TUI runs with upstream integration enabled
- **WHEN** one selected change reaches `pushed`
- **THEN** the TUI remains active in its normal persistent lifecycle
- **AND** a later queued change can execute through the same upstream publication contract
