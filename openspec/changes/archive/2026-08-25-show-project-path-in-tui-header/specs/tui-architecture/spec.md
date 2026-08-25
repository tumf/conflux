## ADDED Requirements

### Requirement: TUI Header Project Identity

The TUI header SHALL show the project path captured from the repository root at startup and SHALL NOT show the workspace concurrency/backend badge. Header rendering MUST use the captured path rather than resolving identity from the process current working directory at render time. When the path exceeds its available header width, the TUI SHALL middle-elide it with one `…`, retaining both a prefix and suffix whenever the budget permits. Width calculation MUST use terminal display columns rather than bytes or Unicode scalar counts. Lifecycle status, dirty-workspace indication, and version presentation SHALL remain available within terminal-width constraints.

#### Scenario: Header identifies the owned project

- **GIVEN** the TUI starts with repository root `/projects/conflux`
- **WHEN** the header is rendered
- **THEN** `/projects/conflux` is visible in the header
- **AND** no `[workspaces:<max>:<backend>]` badge is visible

#### Scenario: Later current-directory changes do not retarget the header

- **GIVEN** the TUI captured repository root `/projects/conflux` at startup
- **AND** the process current working directory later changes to `/tmp`
- **WHEN** the header is rendered again
- **THEN** `/projects/conflux` remains the displayed project path
- **AND** `/tmp` is not adopted as project identity

#### Scenario: Existing header signals survive project identity replacement

- **GIVEN** lifecycle status, a known dirty workspace, and version text are renderable
- **WHEN** the project path is shown in the header
- **THEN** lifecycle status and the dirty badge remain visible when width permits
- **AND** version text remains right-aligned
- **AND** rendering at narrow terminal widths remains bounded and does not panic

#### Scenario: Long project path is middle-elided by display width

- **GIVEN** the captured project path is wider than its available header budget
- **WHEN** the header is rendered
- **THEN** the rendered path contains one `…` between retained path prefix and suffix when the budget permits both
- **AND** the rendered path does not exceed its assigned terminal-column width
- **AND** the suffix receives the extra retained column when the non-ellipsis budget is odd

#### Scenario: Unicode and tiny budgets remain safe

- **GIVEN** a project path contains wide Unicode or combining marks, or its available budget is no wider than the ellipsis
- **WHEN** the header is rendered
- **THEN** truncation does not split a rendered character representation into invalid UTF-8
- **AND** output remains within the assigned terminal-column width
- **AND** rendering does not panic

<!-- Expected canonical result after archive: `tui-architecture` will require the TUI header to identify the startup-captured project path instead of showing workspace concurrency/backend configuration. -->

## MODIFIED Requirements

### Requirement: Local TUI header reports workspace dirty state

The local TUI SHALL observe the Git dirty state of the repository root captured at startup on the existing five-second auto-refresh cadence. A successful observation SHALL classify staged changes, unstaged changes, and untracked files as dirty while excluding ignored files. The TUI header SHALL display a warning-styled `[dirty]` badge only for a known dirty observation and SHALL remove it after a later successful clean observation.

The dirty observation and badge SHALL be process-local presentation state only. They MUST NOT influence reducer state, execution marks, command admission, queue membership, scheduler dispatch, resume routing, acceptance, archive, merge, or any next-action decision.

#### Scenario: Staged change appears in the header

- **GIVEN** the local TUI captured repository root `/repo` at startup
- **AND** `/repo` contains a staged change
- **WHEN** the existing five-second auto-refresh successfully observes Git status
- **THEN** the TUI header displays a red bold `[dirty]` badge after the project path
- **AND** the existing process-mode, project path, and version header content remains visible

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

<!-- Expected canonical result after archive: `tui-architecture` will require an observability-only local TUI dirty badge after the project path, driven by the captured-root five-second refresh and the shared Git dirty predicate. -->
