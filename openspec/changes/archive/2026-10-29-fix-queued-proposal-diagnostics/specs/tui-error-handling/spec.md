## ADDED Requirements

### Requirement: Queued proposal metadata errors use existing error presentation

The existing Changes error row and error details SHALL display the safe proposal diagnostic and correction/Retry remedy using their current visual language. API and client wait SHALL consume the same typed change-local failure. No frontend SHALL infer failure authority by parsing displayed text.

#### Scenario: Error is visible in normal and narrow terminal layouts
- **GIVEN** a proposal failed the existing strict metadata reader before Apply
- **WHEN** Changes and its error details are rendered at 120x30 or 80x24
- **THEN** existing error styling, wrapping, scrolling, focus and keys are preserved
- **AND** the diagnostic includes the proposal location and explicit correction/Retry action
- **AND** no Acceptance/reviewer result is invented
