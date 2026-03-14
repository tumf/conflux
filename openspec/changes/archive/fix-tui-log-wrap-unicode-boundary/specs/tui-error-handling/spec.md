## ADDED Requirements

### Requirement: TUI Logs Rendering Does Not Panic On Unicode Wrap Boundaries

The TUI MUST NOT panic while rendering the Logs view when log messages contain multi-byte UTF-8 characters.
Wrapping logic MUST NOT slice strings at non-character boundaries.

#### Scenario: Wrap a message starting with a multi-byte character

- **GIVEN** a log message starting with `\u{2192}`
- **WHEN** the Logs view wraps the message to a narrow available width (including width 1)
- **THEN** the TUI does not panic
- **AND** the rendered output contains the original message content without corrupting characters
