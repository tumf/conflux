## MODIFIED Requirements

### Requirement: CLI Log Viewer

Conflux SHALL provide a read-only CLI log viewer that helps users locate, print, and follow existing persistent Conflux log files without knowing the internal state-directory layout.

The CLI log viewer SHALL preserve the existing persistent log file layout and SHALL NOT use log contents or log file presence as authoritative workflow-control input for scheduler, resume, acceptance, archive, merge, or next-action decisions.

Conflux SHALL allow operators to configure an absolute Conflux-owned persistent state root without changing process-wide XDG variables. The configured root SHALL take precedence over `XDG_STATE_HOME`; absent configuration SHALL preserve existing XDG and platform-default behavior. Logging, retention cleanup, and the CLI log viewer SHALL resolve the same root. An explicitly configured root that is relative, unavailable, uncreatable, or unwritable SHALL cause startup to fail before orchestration-owned child processes begin, and SHALL NOT silently fall back to another disk.

#### Scenario: Print selected log path without creating logs

- **GIVEN** a user is in a Conflux workspace
- **WHEN** the user runs `cflx logs --path`
- **THEN** Conflux prints the selected log file path for the current project or selected project slug
- **AND** the command does not create a new log file
- **AND** the command does not append to an existing log file
- **AND** the command does not trigger log cleanup as a side effect of viewing

#### Scenario: Print bounded recent log lines

- **GIVEN** a selected Conflux log file exists with more than `N` lines
- **WHEN** the user runs `cflx logs --last N`
- **THEN** Conflux prints at most the last `N` lines from that file
- **AND** Conflux exits successfully without modifying the file

#### Scenario: Default logs command prints recent bounded tail

- **GIVEN** a selected Conflux log file exists
- **WHEN** the user runs `cflx logs` without a viewing mode
- **THEN** Conflux prints a documented bounded number of recent log lines
- **AND** Conflux does not require the user to know the log directory layout

#### Scenario: Follow appended log lines

- **GIVEN** a selected Conflux log file exists
- **WHEN** the user runs `cflx logs --follow`
- **THEN** Conflux prints recent selected log content
- **AND** Conflux streams lines appended after the command starts until the user interrupts it
- **AND** Conflux does not change workflow state while following logs

#### Scenario: Explicit project slug selection

- **GIVEN** multiple project log directories exist under the Conflux log root
- **WHEN** the user runs `cflx logs --project <slug> --path`
- **THEN** Conflux selects the log directory matching `<slug>`
- **AND** Conflux prints the selected log path for that project slug

#### Scenario: Missing selection lists available projects

- **GIVEN** the current project has no matching log file or the requested project slug does not exist
- **WHEN** the user runs `cflx logs`
- **THEN** Conflux returns an actionable error
- **AND** the output lists available project slugs from the log root when any exist

#### Scenario: Configured state root contains persistent logs

- **GIVEN** `state_base_dir` is configured with a writable absolute path
- **WHEN** Conflux initializes persistent logging for a project
- **THEN** it writes logs under `<state_base_dir>/cflx/logs/<project_slug>/`
- **AND** `cflx logs` and retention cleanup resolve that same directory
- **AND** no process-wide XDG variable is changed for child commands

#### Scenario: Configured state root overrides XDG only for Conflux

- **GIVEN** both `state_base_dir` and `XDG_STATE_HOME` are set
- **WHEN** Conflux resolves its persistent state root
- **THEN** Conflux uses `state_base_dir`
- **AND** child Apply, Acceptance, Archive, Resolve, and lifecycle commands retain the original inherited `XDG_STATE_HOME`

#### Scenario: Unset state root preserves current behavior

- **GIVEN** `state_base_dir` is absent or empty
- **WHEN** Conflux resolves its persistent state root
- **THEN** it uses `XDG_STATE_HOME` when available
- **AND** otherwise uses the existing platform fallback

#### Scenario: Invalid configured state root fails closed

- **GIVEN** `state_base_dir` is relative, unavailable, uncreatable, or unwritable
- **WHEN** Conflux starts
- **THEN** startup exits non-zero with an actionable path diagnostic
- **AND** no lifecycle adapter or AI command is started
- **AND** Conflux does not silently write logs to its default internal-disk path

<!-- Expected canonical result after archive: Conflux persistent observability paths can be scoped through config without changing child-process XDG state, while logs remain non-authoritative and all readers/writers share one fail-closed resolver. -->
