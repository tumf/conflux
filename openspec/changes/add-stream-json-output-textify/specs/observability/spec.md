## ADDED Requirements

### Requirement: Stream-JSON Output Textification

When an AI agent command is executed with Claude Code `--output-format stream-json`, the orchestrator MUST convert supported stream-json events into user-facing text for logging.

The orchestrator MUST:

- Detect stream-json output on a per-line basis (newline-delimited JSON objects).
- Extract human-readable text from supported event types (at minimum: `stream_event` text deltas, assistant message text blocks, and final `result`).
- Convert extracted text into line-oriented output:
  - Split by newline and emit one log entry per line.
  - Buffer partial text chunks until a newline boundary is observed.
- Preserve non-stream-json output lines as-is.
- Provide a configuration option to disable stream-json textification.

#### Scenario: stream_event text deltas are rendered as a continuous line stream

- **GIVEN** an agent command produces stream-json lines with `type="stream_event"` and `event.delta.type="text_delta"`
- **WHEN** the orchestrator streams agent output to logs
- **THEN** only the extracted text is emitted to logs (not the JSON events)

#### Scenario: multi-line assistant content is emitted line-by-line

- **GIVEN** an extracted assistant text contains embedded newlines
- **WHEN** the orchestrator emits log output
- **THEN** each newline-separated line is emitted as a separate log entry

#### Scenario: non-stream-json output is preserved

- **GIVEN** an agent command outputs plain text lines (not JSON)
- **WHEN** the orchestrator streams agent output to logs
- **THEN** the same plain text lines are emitted unchanged

#### Scenario: textification can be disabled for troubleshooting

- **GIVEN** stream-json textification is disabled via configuration
- **WHEN** the orchestrator streams agent output to logs
- **THEN** raw stdout/stderr lines are emitted without stream-json conversion
