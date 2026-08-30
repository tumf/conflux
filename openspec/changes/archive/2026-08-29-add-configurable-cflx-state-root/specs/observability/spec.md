## MODIFIED Requirements

### Requirement: CLI Log Viewer

Conflux SHALL provide a read-only CLI log viewer that helps users locate, print, and follow existing persistent Conflux log files without knowing the internal state-directory layout.

The CLI log viewer SHALL preserve the existing persistent log file layout and SHALL NOT use log contents or log file presence as authoritative workflow-control input for scheduler, resume, acceptance, archive, merge, or next-action decisions.

Persistent logs SHALL live under the Conflux-owned state root defined by the `configuration` capability, at `<state root>/logs/<project_slug>/<YYYY-MM-DD>.log`. Logging initialization, retention cleanup, and the CLI log viewer SHALL resolve that root through one shared resolver, so a reader can never point at a directory the writers abandoned. Sharing the resolver is not sufficient on its own: when an invocation names a custom configuration file, the CLI log viewer SHALL merge that same file, so the root it resolves is the root the writers of that same invocation resolve. The CLI log viewer SHALL remain read-only and SHALL list projects only from the currently resolved root; it SHALL NOT migrate, clean, or list logs left under a previously resolved root.

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

#### Scenario: Log viewer lists projects from the currently resolved root only

- **GIVEN** project log directories exist under both a configured state root and a previously resolved root
- **WHEN** the user runs `cflx logs` and no matching log file is selected
- **THEN** the listed project slugs come from the currently resolved root
- **AND** project slugs under the previously resolved root are not listed
- **AND** the command creates, appends to, and cleans nothing under either root

#### Scenario: Log viewer honors a state root supplied through a custom configuration file

- **GIVEN** `state_base_dir` is set only in a configuration file named by the invocation's custom-configuration option, and `XDG_STATE_HOME` points somewhere else
- **WHEN** the user runs the log viewer with that same custom configuration option
- **THEN** the viewer selects, reads, and lists projects under the configured state root
- **AND** it neither reads nor lists anything under the `XDG_STATE_HOME` root

#### Scenario: Log viewer refuses an unusable configured root instead of falling back

- **GIVEN** `state_base_dir` is configured with a value the shared resolver rejects
- **WHEN** the user runs `cflx logs`
- **THEN** the command exits non-zero with an actionable path diagnostic
- **AND** it does not read or list logs from `XDG_STATE_HOME` or the platform default

<!-- Expected canonical result after archive: Conflux persistent observability paths follow the configured Conflux-owned state root, logs remain non-authoritative, and logging, retention cleanup, and the read-only viewer share one resolver so they can never disagree about the current root. -->
