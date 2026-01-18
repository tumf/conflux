## MODIFIED Requirements

### Requirement: Version Display

The CLI SHALL support a `--version` flag to display the application version with UTC build number.

#### Scenario: Display version with --version flag
- **WHEN** user runs `cflx --version`
- **THEN** the application version is displayed in `v<semver>(YYYYMMDDHHmmss)` format
- **AND** the build number uses UTC time
- **AND** the program exits with code 0

#### Scenario: Display version with -V short flag
- **WHEN** user runs `cflx -V`
- **THEN** the application version is displayed in `v<semver>(YYYYMMDDHHmmss)` format

### Requirement: TUI Header Version Display

The TUI header SHALL display the application version with UTC build number in both selection and running modes.

#### Scenario: Version in selection mode header
- **WHEN** TUI is in selection mode
- **THEN** the header displays the application version (e.g., "v0.1.0(20260117113311)")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

#### Scenario: Version in running mode header
- **WHEN** TUI is in running mode
- **THEN** the header displays the application version (e.g., "v0.1.0(20260117113311)")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction
