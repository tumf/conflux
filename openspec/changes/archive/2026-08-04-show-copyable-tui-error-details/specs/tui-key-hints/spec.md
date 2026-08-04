## ADDED Requirements

### Requirement: Error Details Key Hint

The Changes panel SHALL display an `Enter: details` key hint when the cursor is on a change whose display status is `error`. The hint SHALL be available in both Select and Running Changes views and SHALL NOT be shown for a non-error row.

<!-- Expected canonical result after archive: `tui-key-hints` will advertise the error-details popup only when the focused Changes row can open it. -->

#### Scenario: Error row advertises details action

- **GIVEN** the TUI is rendering the Changes view
- **AND** the cursor is on a change whose display status is `error`
- **WHEN** the Changes panel key hints are rendered
- **THEN** the hints SHALL include `Enter: details`

#### Scenario: Non-error row does not advertise details action

- **GIVEN** the TUI is rendering the Changes view
- **AND** the cursor is on a change whose display status is not `error`
- **WHEN** the Changes panel key hints are rendered
- **THEN** the hints SHALL NOT include `Enter: details`
