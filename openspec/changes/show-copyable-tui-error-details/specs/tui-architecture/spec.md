## MODIFIED Requirements

### Requirement: Change List Log Preview

The TUI change list MUST display a single-line preview in the remaining space on the right side of each change row. For a change whose display status is `error`, the preview MUST prefer the retained final change-level diagnostic over every buffered log entry and MUST format it as `Error: <diagnostic>`. This error preview MUST remain available independently of bounded log retention. If the status is `error` but no diagnostic is available, the preview MUST use an explicit fallback such as `Error details unavailable` and MUST NOT present an unrelated ordinary log as the failure reason. For every non-error change, the preview MUST preserve the existing latest-log behavior, including relative time (`just now` for less than 1 minute; `<n><unit> ago` for 1 minute or more), shortened operation header, and message.

Every preview MUST remain single-line and be truncated without wrapping to fit the available display width. Truncation MUST NOT break Unicode character boundaries and MUST NOT panic, even when the content contains CJK characters or emoji. If the available width is less than 10 characters, the preview MUST NOT be displayed. Error previews MUST use readable error styling in both focused and unfocused rows.

<!-- Expected canonical result after archive: `Change List Log Preview` will prefer retained final diagnostics for error rows while preserving existing latest-log previews for non-error rows. -->

#### Scenario: Error preview survives log eviction

- **GIVEN** a change is displayed with status `error`
- **AND** its retained final diagnostic is `Apply failed: stalled after 5 empty WIP commits`
- **AND** the bounded log buffer no longer contains the failure entry
- **WHEN** the TUI renders the Changes list
- **THEN** the row SHALL display `Error: Apply failed: stalled after 5 empty WIP commits` within the available width
- **AND** no retained `LogEntry` SHALL be required to produce that preview

#### Scenario: Error preview takes precedence over ordinary latest log

- **GIVEN** a change is displayed with status `error`
- **AND** the change retains a final diagnostic
- **AND** its latest buffered log is an unrelated ordinary message
- **WHEN** the TUI renders the Changes list
- **THEN** the retained final diagnostic SHALL be shown as the preview
- **AND** the unrelated log SHALL NOT be presented as the error reason

#### Scenario: Missing diagnostic is explicit

- **GIVEN** a change is displayed with status `error`
- **AND** no final diagnostic is available
- **WHEN** the TUI renders the Changes list with sufficient preview width
- **THEN** the preview SHALL state that error details are unavailable
- **AND** it SHALL NOT infer an error reason from an ordinary log

#### Scenario: Error preview truncation is Unicode-safe

- **GIVEN** an error row retains a diagnostic containing Japanese text or emoji
- **AND** the available preview width cannot contain the full diagnostic
- **WHEN** the TUI renders the Changes list
- **THEN** the preview SHALL be truncated to the available display width without wrapping
- **AND** truncation SHALL NOT split a Unicode character or panic

#### Scenario: Non-error preview remains compatible

- **GIVEN** a non-error change has a latest buffered log entry
- **WHEN** the TUI renders the Changes list
- **THEN** the row SHALL continue to show the existing relative-time, operation-header, and message preview
