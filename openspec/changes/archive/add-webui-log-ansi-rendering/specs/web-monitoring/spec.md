## ADDED Requirements

### Requirement: Dashboard log panel ANSI escape rendering

The web dashboard log panel SHALL render ANSI escape sequences in log messages as styled HTML instead of displaying raw escape codes as plain text.

#### Scenario: Log message with ANSI color codes is rendered with color

- **GIVEN** a log entry whose `message` field contains ANSI SGR color escape sequences (e.g. `\x1b[31mERROR\x1b[0m`)
- **WHEN** the dashboard renders the log entry in the Logs panel
- **THEN** the escape sequences are converted to styled `<span>` elements with corresponding foreground/background colors
- **AND** the raw escape code characters (e.g. `[31m`) are not visible to the user

#### Scenario: Log message without ANSI codes is rendered normally

- **GIVEN** a log entry whose `message` field contains no ANSI escape sequences
- **WHEN** the dashboard renders the log entry in the Logs panel
- **THEN** the message text is displayed as-is without any additional markup beyond the existing layout

#### Scenario: Malicious HTML in log message is sanitized

- **GIVEN** a log entry whose `message` field contains HTML tags such as `<script>alert('xss')</script>`
- **WHEN** the dashboard renders the log entry in the Logs panel
- **THEN** the HTML special characters are escaped so that no script execution or DOM injection occurs
- **AND** the literal text of the HTML tag is visible to the user

#### Scenario: ANSI bold and underline decorations are rendered

- **GIVEN** a log entry whose `message` field contains ANSI SGR sequences for bold (`\x1b[1m`) or underline (`\x1b[4m`)
- **WHEN** the dashboard renders the log entry in the Logs panel
- **THEN** the corresponding text is rendered with `font-weight: bold` or `text-decoration: underline` respectively
