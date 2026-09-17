## MODIFIED Requirements

### Requirement: TUI Header Project Identity

The TUI header SHALL show the project path and attached base branch captured from the repository root at startup and SHALL NOT show the workspace concurrency/backend badge. Header rendering MUST use the captured path and branch rather than resolving identity from Git or the process current working directory at render time. The base branch SHALL appear immediately after the project path as `[<branch>]` and before the dirty-workspace indicator. When startup cannot resolve an attached branch, the TUI SHALL omit the branch segment without failing startup. When the path exceeds its available header width, the TUI SHALL reserve the branch and other fixed header segments first, then middle-elide the path with one `…`, retaining both a prefix and suffix whenever the budget permits. Width calculation MUST use terminal display columns rather than bytes or Unicode scalar counts. Lifecycle status, dirty-workspace indication, and version presentation SHALL remain available within terminal-width constraints.

#### Scenario: Header identifies the owned project and base branch

- **GIVEN** the TUI starts with repository root `/projects/conflux` on attached branch `main`
- **WHEN** the header is rendered
- **THEN** `/projects/conflux [main]` is visible in that order
- **AND** the branch appears before any `[dirty]` indicator
- **AND** no `[workspaces:<max>:<backend>]` badge is visible

#### Scenario: Later repository identity changes do not retarget the header

- **GIVEN** the TUI captured repository root `/projects/conflux` and attached branch `main` at startup
- **AND** the process current working directory or repository branch later changes
- **WHEN** the header is rendered again
- **THEN** `/projects/conflux [main]` remains the displayed startup identity
- **AND** rendering does not query Git or the ambient current directory

#### Scenario: Missing attached branch preserves startup and current header behavior

- **GIVEN** startup cannot resolve an attached branch because the repository is detached or branch lookup is unavailable
- **WHEN** the TUI initializes and renders its header
- **THEN** startup succeeds
- **AND** the project path remains visible
- **AND** no empty, guessed, or placeholder branch segment is rendered

#### Scenario: Existing header signals survive branch identity addition

- **GIVEN** lifecycle status, a captured base branch, a known dirty workspace, and version text are renderable
- **WHEN** the project path and branch are shown in the header
- **THEN** lifecycle status, branch, and dirty badge remain visible when width permits
- **AND** version text remains right-aligned
- **AND** rendering at narrow terminal widths remains bounded and does not panic

#### Scenario: Long project path yields width to the branch segment

- **GIVEN** the captured project path is wider than the header budget remaining after the branch and other fixed segments are reserved
- **WHEN** the header is rendered
- **THEN** the rendered path contains one `…` between retained path prefix and suffix when the budget permits both
- **AND** the captured branch remains visible when its full segment fits the fixed-segment area
- **AND** the rendered path does not exceed its assigned terminal-column width
- **AND** the suffix receives the extra retained column when the non-ellipsis budget is odd

#### Scenario: Unicode and tiny budgets remain safe

- **GIVEN** a project path or branch contains wide Unicode or combining marks, or the path budget is no wider than the ellipsis
- **WHEN** the header is rendered
- **THEN** rendering does not split a rendered character representation into invalid UTF-8
- **AND** output remains within the assigned terminal-column width
- **AND** rendering does not panic

<!-- Expected canonical result after archive: `tui-architecture` will require the TUI header to show the startup-captured base branch immediately after the startup-captured project path, while preserving bounded display-width elision and existing header signals. -->
