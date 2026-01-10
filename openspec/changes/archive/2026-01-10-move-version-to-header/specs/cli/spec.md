# cli Spec Delta

## NEW Requirements

### Requirement: TUI Header Version Display

The TUI header SHALL display the application version in both selection and running modes.

#### Scenario: Version in selection mode header
- **WHEN** TUI is in selection mode
- **THEN** the header displays the application version (e.g., "v0.1.0")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

#### Scenario: Version in running mode header
- **WHEN** TUI is in running mode
- **THEN** the header displays the application version (e.g., "v0.1.0")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

## REMOVED Requirements

### Requirement: TUI Footer Version Display

The TUI selection mode footer no longer displays the application version. Version display has been moved to the header.

#### Scenario: Footer without version in selection mode
- **WHEN** TUI is in selection mode
- **THEN** the footer does NOT display the application version
- **AND** the footer contains only status information and guidance messages

