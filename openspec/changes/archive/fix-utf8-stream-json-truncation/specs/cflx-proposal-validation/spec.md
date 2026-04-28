## MODIFIED Requirements

### Requirement: Native validator owns behavior-centric proposal checks

Conflux-native textification and summary helpers used during proposal-oriented command execution MUST remain resilient when truncating valid UTF-8 text for display. Any bounded display truncation in these helpers MUST preserve character boundaries so logging and UI-adjacent summaries do not panic on multi-byte input.

#### Scenario: Assistant tool summary truncation with multi-byte UTF-8 does not panic

- **GIVEN** a stream-json assistant tool summary contains a long UTF-8 string value such as a `filePath`, `pattern`, `url`, `prompt`, or `args` field
- **AND** the configured summary limit would otherwise cut through a multi-byte character
- **WHEN** the summary is rendered for display
- **THEN** rendering does not panic
- **AND** the summary remains truncated and human-readable

#### Scenario: Tool-result summary truncation with multi-byte UTF-8 does not panic

- **GIVEN** a stream-json tool-result payload contains long UTF-8 text content
- **AND** the configured summary limit would otherwise cut through a multi-byte character
- **WHEN** the tool-result summary is rendered for display
- **THEN** rendering does not panic
- **AND** the displayed summary is truncated on UTF-8 character boundaries
