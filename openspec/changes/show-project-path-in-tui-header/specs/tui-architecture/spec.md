## ADDED Requirements

### Requirement: TUI Header Project Identity

The TUI header SHALL show the project path captured from the repository root at startup and SHALL NOT show the workspace concurrency/backend badge. Header rendering MUST use the captured path rather than resolving identity from the process current working directory at render time. Lifecycle status, dirty-workspace indication, and version presentation SHALL remain available within terminal-width constraints.

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

<!-- Expected canonical result after archive: `tui-architecture` will require the TUI header to identify the startup-captured project path instead of showing workspace concurrency/backend configuration. -->
