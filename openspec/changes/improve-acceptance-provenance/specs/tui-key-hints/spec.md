## MODIFIED Requirements

### Requirement: Error Details Key Hint

The Changes panel SHALL display an `Enter: details` key hint when the cursor is on a change whose display status is `error`, `stalled`, or `blocked` with blocker kind `external`. The hint SHALL be available in both Select and Running Changes views and SHALL NOT be shown for any other row.

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


#### Scenario: Stalled and externally blocked rows advertise details action

- **GIVEN** the TUI is rendering the Changes view
- **AND** the cursor is on a change whose display status is `stalled`, or `blocked` with blocker kind `external`
- **WHEN** the Changes panel key hints are rendered
- **THEN** the hints SHALL include `Enter: details`
- **AND** a dependency-`blocked` row SHALL NOT include it
