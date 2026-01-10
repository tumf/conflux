## MODIFIED Requirements

### Requirement: System Prompt for Apply and Archive Commands

The orchestrator SHALL support `{prompt}` placeholder in both `apply_command` and `archive_command` templates, allowing system-provided instructions to be injected into agent commands.

The orchestrator SHALL provide `apply_prompt` and `archive_prompt` configuration options to define user-customizable prompt values.

The orchestrator SHALL include a hardcoded system prompt for apply commands that is always appended after `apply_prompt`. This system prompt enforces non-negotiable task management rules and cannot be disabled by user configuration.

The `{prompt}` placeholder for apply commands SHALL be expanded as: `apply_prompt` + hardcoded system prompt + history context (if any), concatenated with newlines.

The hardcoded system prompt SHALL contain:
- "Remove out-of-scope tasks."
- "Remove tasks that wait for or require user action."

#### Scenario: Apply command prompt structure

- **GIVEN** `apply_command` is configured as `"agent apply {change_id} {prompt}"`
- **AND** `apply_prompt` is configured as `"Focus on implementation."`
- **WHEN** applying change `add-feature`
- **THEN** the `{prompt}` expands to: `"Focus on implementation.\n\nRemove out-of-scope tasks. Remove tasks that wait for or require user action."`

#### Scenario: Apply command with empty user prompt

- **GIVEN** `apply_command` is configured as `"agent apply {change_id} {prompt}"`
- **AND** `apply_prompt` is empty or NOT configured
- **WHEN** applying change `add-feature`
- **THEN** the `{prompt}` expands to the hardcoded system prompt only

#### Scenario: Apply command with history context

- **GIVEN** `apply_command` is configured as `"agent apply {change_id} {prompt}"`
- **AND** `apply_prompt` is configured as `"Focus on implementation."`
- **AND** there is a previous failed apply attempt for the change
- **WHEN** applying change `add-feature`
- **THEN** the `{prompt}` expands to: user prompt + system prompt + history context

#### Scenario: Archive command unchanged

- **GIVEN** `archive_command` is configured as `"agent archive {change_id} {prompt}"`
- **AND** `archive_prompt` is configured as `"Verify completion."`
- **WHEN** archiving change `add-feature`
- **THEN** the `{prompt}` expands to `archive_prompt` only (no hardcoded system prompt for archive)
