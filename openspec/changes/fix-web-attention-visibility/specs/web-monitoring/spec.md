## ADDED Requirements

### Requirement: Hidden attention summaries obey rendered visibility
The Web console SHALL hide the attention summary when the current coherent presentation has no attention changes, including after recovery/archive, under the actual shipped stylesheet. A nonhidden attention summary SHALL retain its current flex appearance and show current attention names. Visibility SHALL NOT alter workflow or command authority.

#### Scenario: Current attention is removed
- **GIVEN** a rendered attention summary and a coherent same-console update removing the attention rows or changing them to non-attention
- **WHEN** the production renderer sets the summary hidden
- **THEN** computed display is none and there is no rendered attention box, even if prior text remains in its DOM
- **AND** no command is submitted

#### Scenario: Attention appears again
- **GIVEN** a hidden attention summary with no attention rows
- **WHEN** a coherent update introduces a current attention row
- **THEN** the summary uses the unchanged flex appearance and current row names
- **AND** no recovery or lifecycle command is automatically submitted
