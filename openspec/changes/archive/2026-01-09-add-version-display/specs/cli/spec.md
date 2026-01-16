# cli Specification Delta

## ADDED Requirements

### Requirement: Version Display

The CLI SHALL support a `--version` flag to display the application version.

#### Scenario: Display version with --version flag
- **WHEN** user runs `cflx --version`
- **THEN** the application version from Cargo.toml is displayed
- **AND** the program exits with code 0

#### Scenario: Display version with -V short flag
- **WHEN** user runs `cflx -V`
- **THEN** the application version is displayed (same as `--version`)

### Requirement: TUI Footer Version Display

The TUI selection mode footer SHALL display the application version.

#### Scenario: Version in selection mode footer
- **WHEN** TUI is in selection mode
- **THEN** the footer displays the application version (e.g., "v0.1.0")
- **AND** the version is displayed on the right side of the footer
- **AND** the version text uses a muted/gray color to avoid distraction
