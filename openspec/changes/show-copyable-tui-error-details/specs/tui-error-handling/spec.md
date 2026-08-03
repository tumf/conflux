## ADDED Requirements

### Requirement: Copyable Change Error Details

The TUI SHALL expose the retained final diagnostic for a change-level `error` through an Error Details popup that is independent of the bounded log buffer. Pressing `Enter` on an `error` row SHALL open the popup; `Enter` on non-error rows SHALL preserve its existing behavior. The popup SHALL display the change ID and complete untruncated diagnostic, support popup-local scrolling for content that exceeds its body, and visibly advertise scroll, `c: copy`, and `Esc: close` controls.

While visible, the Error Details popup SHALL own its handled input so popup scroll, copy, and close keys do not move or activate the underlying Changes list, Logs panel, or another interaction modal. Pressing `c` SHALL request copying plain text formatted exactly as `Change: <id>\nError: <diagnostic>` to the OS clipboard. Copy success or failure SHALL be reported inside the popup, and either result SHALL leave the popup open with the diagnostic intact. Clipboard access SHALL be testable through an injected implementation so automated tests do not alter the operator's clipboard.

The retained diagnostic and popup are observability presentation only and MUST NOT become workflow-control inputs for scheduling, retry routing, acceptance, archive, or merge decisions.

<!-- Expected canonical result after archive: `tui-error-handling` will require a scrollable, copyable Error Details popup for change-level error rows, independent of log retention and workflow control. -->

#### Scenario: Enter opens complete details after logs are gone

- **GIVEN** a change row is in `error`
- **AND** its final diagnostic is retained in change presentation state
- **AND** the bounded log buffer no longer contains its failure log
- **WHEN** the user presses `Enter` on that row
- **THEN** the TUI SHALL open an Error Details popup
- **AND** the popup SHALL display the change ID and complete final diagnostic

#### Scenario: Popup scrolling does not move underlying views

- **GIVEN** an Error Details popup contains more lines than its visible body
- **WHEN** the user presses a supported scroll key such as `Down`, `j`, or `PageDown`
- **THEN** the popup content SHALL scroll
- **AND** the underlying change cursor and Logs-panel position SHALL remain unchanged

#### Scenario: Copy succeeds with stable plain text

- **GIVEN** the Error Details popup is open for change `alpha`
- **AND** its diagnostic is `Apply failed: stalled`
- **WHEN** the user presses `c`
- **THEN** the clipboard SHALL receive exactly `Change: alpha\nError: Apply failed: stalled`
- **AND** the popup SHALL remain open
- **AND** the popup SHALL show copy-success feedback

#### Scenario: Clipboard failure preserves details

- **GIVEN** the Error Details popup is open
- **AND** the clipboard implementation returns an error
- **WHEN** the user presses `c`
- **THEN** the popup SHALL remain open with the complete diagnostic intact
- **AND** the popup SHALL show actionable copy-failure feedback

#### Scenario: Escape closes details without workflow transition

- **GIVEN** the Error Details popup is open
- **WHEN** the user presses `Esc`
- **THEN** the popup SHALL close
- **AND** no workflow-control state SHALL change because of closing it

#### Scenario: Non-error Enter behavior is unchanged

- **GIVEN** the cursor is on a change whose status is not `error`
- **WHEN** the user presses `Enter`
- **THEN** the TUI SHALL perform the pre-existing Enter action for that row
- **AND** it SHALL NOT open the Error Details popup
