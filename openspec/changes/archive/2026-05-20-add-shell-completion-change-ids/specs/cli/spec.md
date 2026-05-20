## ADDED Requirements

### Requirement: Shell Completion Generation

The CLI SHALL provide a `completion` subcommand that generates shell completion scripts for supported shells without starting orchestration, TUI, server, or workspace-dependent runtime behavior during script generation.

<!-- Expected canonical result after archive: cli spec documents `cflx completion <shell>` as a side-effect-free script generation surface for zsh, bash, fish, and powershell. -->

#### Scenario: Generate completion script for supported shell

- **WHEN** user runs `cflx completion zsh`
- **OR** user runs `cflx completion bash`
- **OR** user runs `cflx completion fish`
- **OR** user runs `cflx completion powershell`
- **THEN** `cflx` prints a non-empty completion script to stdout
- **AND** exits with status code 0
- **AND** does not create or append Conflux log files
- **AND** does not require an OpenSpec workspace

#### Scenario: Completion output is script-only

- **WHEN** user runs `cflx completion <supported-shell>`
- **THEN** stdout contains the generated completion script
- **AND** stdout does not contain startup logs, status banners, or human-readable explanatory text

#### Scenario: Unsupported completion shell is rejected

- **WHEN** user runs `cflx completion unsupported-shell`
- **THEN** the command exits with a non-zero status code
- **AND** the error output lists the supported shell values

### Requirement: Dynamic Change ID Completion

Generated shell completion scripts SHALL provide workspace-local OpenSpec change ID candidates for commands and options that accept change IDs. Candidate lookup SHALL be side-effect free and SHALL read only workspace-local `openspec/changes/` state.

<!-- Expected canonical result after archive: cli spec documents dynamic OpenSpec change-id completion for `run --change`, `openspec show`, `openspec validate`, and `openspec archive`, including active/archived scoping and dated archive normalization. -->

#### Scenario: Run change option completes active changes

- **GIVEN** active changes exist under `openspec/changes/`
- **WHEN** the user requests shell completion for `cflx run --change <prefix>`
- **THEN** completion candidates include matching active change IDs
- **AND** archived changes are not included

#### Scenario: Run change option completes comma-separated values

- **GIVEN** active changes `alpha`, `beta`, and `gamma` exist
- **WHEN** the user requests shell completion for `cflx run --change alpha,b`
- **THEN** completion candidates are evaluated against the current comma-separated token `b`
- **AND** `beta` is offered as a candidate
- **AND** already-entered values such as `alpha` are not duplicated when the shell integration can suppress duplicates

#### Scenario: Openspec show completes active and archived changes

- **GIVEN** active change `active-change` exists
- **AND** archived change `archived-change` exists under `openspec/changes/archive/`
- **WHEN** the user requests shell completion for `cflx openspec show <prefix>`
- **THEN** completion candidates include matching active changes
- **AND** completion candidates include matching archived changes

#### Scenario: Openspec show normalizes dated archived change IDs

- **GIVEN** archived change directory `openspec/changes/archive/2026-04-28-archived-change` exists
- **WHEN** the user requests shell completion for `cflx openspec show archived`
- **THEN** the candidate is `archived-change`
- **AND** the date prefix is not included in the displayed logical change ID

#### Scenario: Openspec validate completes active changes only

- **GIVEN** active and archived changes exist
- **WHEN** the user requests shell completion for `cflx openspec validate <prefix>`
- **THEN** completion candidates include matching active changes
- **AND** archived changes are not included
- **AND** invoking `cflx openspec validate` with no change ID remains valid

#### Scenario: Openspec archive completes active changes only

- **GIVEN** active and archived changes exist
- **WHEN** the user requests shell completion for `cflx openspec archive <prefix>`
- **THEN** completion candidates include matching active changes
- **AND** archived changes are not included

#### Scenario: Change ID candidate lookup is side-effect free

- **WHEN** a generated completion script asks `cflx` for change ID candidates
- **THEN** the candidate lookup reads only workspace-local `openspec/changes/` state
- **AND** does not initialize runtime logging
- **AND** does not create, update, or delete workflow state
- **AND** exits with status code 0 and empty stdout when no workspace or no candidates exist
