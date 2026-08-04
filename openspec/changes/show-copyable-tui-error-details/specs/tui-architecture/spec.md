## MODIFIED Requirements

### Requirement: Change List Log Preview

The TUI change list MUST display a single-line preview in the remaining space on the right side of each change row. For a change whose display status is `error`, the preview MUST prefer the retained final change-level diagnostic over every buffered log entry and MUST format it as `Error: <diagnostic>`. This error preview MUST remain available independently of bounded log retention. If the status is `error` but no diagnostic is available, the preview MUST use an explicit fallback such as `Error details unavailable` and MUST NOT present an unrelated ordinary log as the failure reason. For every non-error change, the preview MUST display the latest log entry and include its relative time (`just now` for less than 1 minute; `<n><unit> ago` for 1 minute or more, e.g., `2m ago`, `3h ago`, with values truncated (no rounding up)), the shortened header format `[operation:{iteration}]` or `[operation]`, and the message.

Every preview MUST remain single-line and be truncated without wrapping to fit the available display width. Truncation MUST NOT break Unicode character boundaries and MUST NOT panic, even when the content contains CJK characters or emoji. Error previews MUST use readable error styling in both focused and unfocused rows.

- For relative times of 1 minute or more on non-error log previews, the display MUST include up to 2 units. Units MUST be `d` / `h` / `m`, formatted as space-separated units such as `1d 12h ago` or `3h 20m ago`. Values MUST be truncated (no rounding up).
- If no log entry exists for a non-error change, the preview MUST NOT be displayed.
- If the available width for the preview is less than 10 characters, the preview MUST NOT be displayed.
- The relative time for a non-error log preview MUST be computed at render time from the log entry creation time and the current time, and the display MUST update at 1-second granularity.

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

#### Scenario: Change list displays preview with relative time for latest log

- **GIVEN** a non-error change has a log entry from 2 minutes ago (`operation="resolve"`, `iteration=1`)
- **WHEN** the TUI renders the change list
- **THEN** the change row SHALL display `2m ago [resolve:1]` and the latest log message on the same line

#### Scenario: Change list does not display preview when no logs exist

- **GIVEN** a non-error change has no log entries
- **WHEN** the TUI renders the change list
- **THEN** the change row SHALL NOT display a log preview

#### Scenario: Change list does not display preview when preview width is insufficient

- **GIVEN** the available width for the log or error preview is less than 10 characters
- **WHEN** the TUI renders the change list
- **THEN** the change list SHALL NOT display a preview

#### Scenario: Change list displays up to two units for relative time

- **GIVEN** a non-error change has a log entry from 1 day and 12 hours ago (`operation="apply"`, `iteration=3`)
- **WHEN** the TUI renders the change list
- **THEN** the change row SHALL display `1d 12h ago [apply:3]` and the latest log message on the same line

#### Scenario: Relative time updates as time elapses

- **GIVEN** a non-error change has a log entry from 59 seconds ago
- **WHEN** the TUI renders the change list
- **THEN** the change row SHALL display `just now` as the relative time
- **WHEN** 2 seconds pass and the TUI re-renders the change list
- **THEN** the change row SHALL display `1m ago` as the relative time

#### Scenario: Log preview truncation is Unicode-safe for Japanese text

- **GIVEN** the latest log message for a non-error change contains Japanese text such as `追記済みです。`
- **AND** the available preview width is insufficient to display the full message
- **WHEN** the TUI renders the change list
- **THEN** the log preview SHALL be truncated without breaking Unicode character boundaries
- **AND** the TUI SHALL continue rendering without panicking
