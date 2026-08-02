## MODIFIED Requirements

### Requirement: QR Code Popup Display

The TUI SHALL provide a QR code popup overlay to display the Web UI access URL when web monitoring is enabled. The popup SHALL be represented as presentation state layered over the current execution mode and SHALL NOT replace, capture, restore, or otherwise mutate that execution mode.

#### Scenario: Display QR popup with W key

- **GIVEN** the TUI is in select, running, or stopped execution mode
- **AND** web monitoring is enabled (web_url is Some)
- **WHEN** the user presses `w` key
- **THEN** the TUI SHALL display a centered popup overlay
- **AND** the popup SHALL contain a QR code encoding the Web UI URL
- **AND** the popup SHALL display the URL text below the QR code
- **AND** the popup title SHALL be "Web UI QR Code"
- **AND** the underlying execution mode SHALL remain unchanged

#### Scenario: Close QR popup with Esc key

- **GIVEN** the TUI is displaying the QR popup overlay
- **WHEN** the user presses `Esc` key
- **THEN** the popup SHALL close
- **AND** the TUI SHALL expose the current underlying execution mode
- **AND** the TUI SHALL NOT restore a stale execution mode captured when the popup opened

#### Scenario: Close QR popup with any key

- **GIVEN** the TUI is displaying the QR popup overlay
- **WHEN** the user presses any key other than `Esc`
- **THEN** the popup SHALL close
- **AND** the TUI SHALL expose the current underlying execution mode
- **AND** the key SHALL NOT trigger an action in the underlying view

#### Scenario: W key ignored when web monitoring disabled

- **GIVEN** the TUI is in select, running, or stopped execution mode
- **AND** web monitoring is disabled (web_url is None)
- **WHEN** the user presses `w` key
- **THEN** the TUI SHALL NOT display the QR popup
- **AND** no execution or modal state change SHALL occur
