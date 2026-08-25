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
