## ADDED Requirements

### Requirement: Local TUI header reports workspace dirty state

The local TUI SHALL observe the Git dirty state of the repository root captured at startup on the existing five-second auto-refresh cadence. A successful observation SHALL classify staged changes, unstaged changes, and untracked files as dirty while excluding ignored files. The TUI header SHALL display a warning-styled `[dirty]` badge only for a known dirty observation and SHALL remove it after a later successful clean observation.

The dirty observation and badge SHALL be process-local presentation state only. They MUST NOT influence reducer state, execution marks, command admission, queue membership, scheduler dispatch, resume routing, acceptance, archive, merge, or any next-action decision.

#### Scenario: Staged change appears in the header

- **GIVEN** the local TUI captured repository root `/repo` at startup
- **AND** `/repo` contains a staged change
- **WHEN** the existing five-second auto-refresh successfully observes Git status
- **THEN** the TUI header displays a red bold `[dirty]` badge after the workspaces badge
- **AND** the existing process-mode, workspaces, and version header content remains visible

#### Scenario: Unstaged change appears in the header

- **GIVEN** the captured repository root contains an unstaged tracked-file change
- **WHEN** the existing auto-refresh successfully observes Git status
- **THEN** the TUI header displays `[dirty]`

#### Scenario: Untracked file appears in the header

- **GIVEN** the captured repository root contains an untracked file
- **AND** repository or user configuration would otherwise suppress untracked status output
- **WHEN** the existing auto-refresh uses the shared dirty-state Git predicate
- **THEN** the TUI header displays `[dirty]`

#### Scenario: Ignored files do not appear as dirty

- **GIVEN** the captured repository root is clean except for ignored files
- **WHEN** the existing auto-refresh successfully observes Git status
- **THEN** the TUI header does not display `[dirty]`

#### Scenario: Successful clean refresh removes the badge

- **GIVEN** the latest successful observation is dirty and the TUI header displays `[dirty]`
- **AND** the workspace is subsequently cleaned
- **WHEN** the next existing five-second auto-refresh successfully observes the clean state
- **THEN** the TUI removes `[dirty]` without restarting
- **AND** no orchestration state changes solely because the badge disappeared

#### Scenario: Failed observation preserves the last successful state

- **GIVEN** the latest successful workspace observation is dirty
- **WHEN** a later Git status observation fails
- **THEN** the TUI preserves the dirty presentation state
- **AND** it does not replace the state with clean
- **AND** it emits a bounded warning without stopping refresh or orchestration

#### Scenario: Unknown initial state makes no cleanliness claim

- **GIVEN** the TUI has not completed a successful workspace dirty observation
- **WHEN** the header is rendered
- **THEN** `[dirty]` is omitted
- **AND** the unknown observation is not treated as clean evidence for any workflow decision

#### Scenario: Refresh remains bound to the captured repository root

- **GIVEN** the local TUI captured repository root `/repo` at startup
- **AND** the process current working directory later changes to `/other`
- **AND** `/repo` is dirty while `/other` is clean
- **WHEN** the existing five-second auto-refresh observes workspace dirty state
- **THEN** the TUI displays `[dirty]` from `/repo`
- **AND** it does not derive the badge from `/other`

#### Scenario: Dirty badge is observability-only

- **GIVEN** two otherwise identical TUI states differ only in their workspace dirty presentation observation
- **WHEN** reducer status, execution marks, command admission, queue routing, resume, acceptance, archive, merge, and next-action behavior are evaluated
- **THEN** both states produce identical workflow behavior
- **AND** only the rendered header badge differs

<!-- Expected canonical result after archive: `tui-architecture` will require an observability-only local TUI dirty badge driven by the captured-root five-second refresh and the shared Git dirty predicate. -->
