# tui-qr-popup Specification

## Purpose

Defines the QR code popup feature for displaying Web UI access URL in TUI mode.

## Requirements

### Requirement: QR Code Popup Display

The TUI SHALL provide a QR code popup overlay to display the Web UI access URL when web monitoring is enabled.

#### Scenario: Display QR popup with W key

- **GIVEN** the TUI is in select, running, or stopped mode
- **AND** web monitoring is enabled (web_url is Some)
- **WHEN** the user presses `w` key
- **THEN** the TUI SHALL display a centered popup overlay
- **AND** the popup SHALL contain a QR code encoding the Web UI URL
- **AND** the popup SHALL display the URL text below the QR code
- **AND** the popup title SHALL be "Web UI QR Code"

#### Scenario: Close QR popup with Esc key

- **GIVEN** the TUI is displaying the QR popup overlay
- **WHEN** the user presses `Esc` key
- **THEN** the popup SHALL close
- **AND** the TUI SHALL return to the previous mode

#### Scenario: Close QR popup with any key

- **GIVEN** the TUI is displaying the QR popup overlay
- **WHEN** the user presses any key other than `Esc`
- **THEN** the popup SHALL close
- **AND** the TUI SHALL return to the previous mode

#### Scenario: W key ignored when web monitoring disabled

- **GIVEN** the TUI is in select, running, or stopped mode
- **AND** web monitoring is disabled (web_url is None)
- **WHEN** the user presses `w` key
- **THEN** the TUI SHALL NOT display the QR popup
- **AND** no mode change SHALL occur

### Requirement: QR Code Generation

The TUI SHALL generate valid QR codes from Web UI URLs using ASCII/Unicode rendering.

#### Scenario: Generate QR code from valid URL

- **GIVEN** a valid Web UI URL like "http://192.168.1.100:54321"
- **WHEN** QR code generation is requested
- **THEN** a valid QR code string SHALL be generated
- **AND** the QR code SHALL be scannable by standard QR code readers
- **AND** the QR code SHALL use Unicode block characters for rendering

#### Scenario: Handle QR generation failure gracefully

- **GIVEN** a URL that cannot be encoded as QR code (e.g., extremely long URL)
- **WHEN** QR code generation fails
- **THEN** the popup SHALL display the URL text only
- **AND** an error message SHALL be shown indicating QR generation failed

### Requirement: Key Hint Integration

The TUI SHALL display appropriate key hints for the QR popup feature based on web monitoring state.

#### Scenario: Show W key hint when web enabled

- **GIVEN** the TUI is in select, running, or stopped mode
- **AND** web monitoring is enabled
- **THEN** the key hints SHALL include "w: QR"

#### Scenario: Hide W key hint when web disabled

- **GIVEN** the TUI is in select, running, or stopped mode
- **AND** web monitoring is disabled
- **THEN** the key hints SHALL NOT include "w: QR"

#### Scenario: Show close hint in QR popup mode

- **GIVEN** the TUI is displaying the QR popup overlay
- **THEN** the key hints SHALL show "Esc: close" or "Any key: close"

### Requirement: Web URL State Management

The TUI SHALL receive the actual Web UI URL after server startup, including the auto-assigned port.

#### Scenario: Set web URL after server binds to auto-assigned port

- **GIVEN** the orchestrator is started with `--web` flag
- **AND** the web server binds to an auto-assigned port
- **AND** the bind address is "0.0.0.0"
- **WHEN** the web server successfully starts
- **THEN** AppState.web_url SHALL be set to "http://<local-ip>:<actual-port>"
- **AND** the actual port SHALL be the port assigned by the OS

#### Scenario: Set web URL with localhost bind

- **GIVEN** the orchestrator is started with `--web` flag
- **AND** the bind address is "127.0.0.1"
- **WHEN** the web server successfully starts
- **THEN** AppState.web_url SHALL be set to "http://localhost:<actual-port>"

#### Scenario: Web URL is None without web flag

- **GIVEN** the orchestrator is started without `--web` flag
- **WHEN** the TUI initializes
- **THEN** AppState.web_url SHALL be None

### Requirement: Access URL Construction

The TUI SHALL construct an accessible URL from bind address and actual port.

#### Scenario: Convert 0.0.0.0 bind to local IP

- **GIVEN** the web server binds to "0.0.0.0"
- **AND** the local network IP is "192.168.1.100"
- **AND** the actual assigned port is 54321
- **WHEN** the access URL is constructed
- **THEN** the URL SHALL be "http://192.168.1.100:54321"

#### Scenario: Keep localhost for 127.0.0.1 bind

- **GIVEN** the web server binds to "127.0.0.1"
- **AND** the actual assigned port is 54321
- **WHEN** the access URL is constructed
- **THEN** the URL SHALL be "http://localhost:54321"

#### Scenario: Fallback to localhost when local IP unavailable

- **GIVEN** the web server binds to "0.0.0.0"
- **AND** no local network IP can be determined
- **AND** the actual assigned port is 54321
- **WHEN** the access URL is constructed
- **THEN** the URL SHALL be "http://localhost:54321"
- **AND** a warning SHALL be logged about limited accessibility

### Requirement: QR Popup Layout

The TUI SHALL render the QR popup with proper centering and sizing.

#### Scenario: Popup centered on screen

- **GIVEN** the TUI terminal has sufficient size
- **WHEN** the QR popup is displayed
- **THEN** the popup SHALL be horizontally and vertically centered
- **AND** the popup width SHALL be approximately 60% of terminal width
- **AND** the popup height SHALL accommodate the QR code and URL text

#### Scenario: Handle small terminal gracefully

- **GIVEN** the terminal size is too small to display the full QR code
- **WHEN** the QR popup is displayed
- **THEN** the popup SHALL display with reduced QR code size if possible
- **OR** display URL text only with a message about terminal size
