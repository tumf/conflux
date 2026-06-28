## MODIFIED Requirements

### Requirement: run Subcommand

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop only when the operator provides an explicit target mode: `--all`, one or more positional change IDs, or the legacy `--change` option. Positional IDs and legacy `--change` values SHALL be normalized into the same explicit change ID target list. `--all` SHALL target all current changes from the initial run snapshot. The target modes SHALL be mutually exclusive.

<!-- Expected canonical result after archive: `run Subcommand` will require explicit target selection for non-interactive run mode, add positional ID support, keep legacy `--change`, and remove partial execution for unknown IDs. -->

#### Scenario: Run with positional changes

- **WHEN** user runs `cflx run a b c`
- **THEN** only changes `a`, `b`, and `c` are processed
- **AND** the snapshot log shows only `a`, `b`, and `c`
- **AND** the behavior is equivalent to starting the TUI with those three changes selected for execution

#### Scenario: Run all changes explicitly

- **WHEN** user runs `cflx run --all`
- **THEN** all current changes from the initial run snapshot are targeted
- **AND** the behavior is equivalent to using the TUI `x` bulk execution mark before starting

#### Scenario: Bare run is rejected

- **WHEN** user runs `cflx run` without `--all`, positional change IDs, or `--change`
- **THEN** orchestration does not start
- **AND** the command exits with an error explaining that `--all` or at least one change ID is required

#### Scenario: Run with specific legacy change option

- **WHEN** user runs `cflx run --change <id>`
- **THEN** only the specified change is processed
- **AND** the specified change is handled through the same normalized explicit target list as positional IDs

#### Scenario: Run with comma-separated legacy changes

- **WHEN** user runs `cflx run --change a,b,c`
- **THEN** only changes `a`, `b`, `c` are processed
- **AND** the snapshot log shows only `a`, `b`, `c`
- **AND** duplicate and unknown validation matches positional ID validation

#### Scenario: Run with non-existent change

- **WHEN** user runs `cflx run nonexistent`
- **AND** no change named `nonexistent` exists in the initial run snapshot
- **THEN** orchestration does not start
- **AND** the command exits with an error naming `nonexistent`
- **AND** no partial subset of requested changes is processed

#### Scenario: Run with mixed valid and invalid changes

- **WHEN** user runs `cflx run a nonexistent c`
- **AND** `a` and `c` exist but `nonexistent` does not
- **THEN** orchestration does not start
- **AND** the command exits with an error naming `nonexistent`
- **AND** neither `a` nor `c` is processed as a partial subset

#### Scenario: Duplicate explicit changes are rejected

- **WHEN** user runs `cflx run a a`
- **THEN** orchestration does not start
- **AND** the command exits with an error naming duplicate change `a`

#### Scenario: Target modes are mutually exclusive

- **WHEN** user combines `--all` with positional IDs or `--change`
- **OR** combines positional IDs with `--change`
- **THEN** parsing or validation fails before orchestration starts
- **AND** the error explains that exactly one target mode must be used

#### Scenario: Parallel dry-run honors explicit targets

- **WHEN** user runs `cflx run --parallel --dry-run a c`
- **THEN** the dry-run plan includes only `a` and `c`
- **AND** unrequested changes are excluded from dependency grouping output

#### Scenario: Parallel execution honors explicit targets

- **WHEN** user runs `cflx run --parallel a c`
- **THEN** parallel execution starts only for `a` and `c`
- **AND** unrequested changes are not scheduled into worktrees

#### Scenario: Successful run exits promptly

- **GIVEN** orchestration completes successfully and no restart was explicitly requested
- **WHEN** `cflx run --all` or `cflx run <change-id>...` logs successful completion
- **THEN** the command exits promptly with status code 0
- **AND** it does not wait for an additional stop signal before terminating

### Requirement: Default TUI Launch

When launched without a subcommand, the interactive TUI SHALL be displayed. The non-interactive `run` subcommand SHALL require explicit target selection and SHALL NOT use bare `cflx run` as a backward-compatible all-changes shortcut.

<!-- Expected canonical result after archive: Default TUI Launch will preserve bare `cflx` TUI launch while removing the bare `cflx run` compatibility scenario. -->

#### Scenario: Launch without subcommand

- **WHEN** user runs `cflx` without arguments
- **THEN** the interactive TUI is launched
- **AND** the change list is displayed in selection mode

#### Scenario: Launch with run subcommand requires targets

- **WHEN** user runs `cflx run` without explicit targets
- **THEN** the orchestration loop is not executed
- **AND** the command exits with guidance to run `cflx run --all` or `cflx run <change-id>...`
