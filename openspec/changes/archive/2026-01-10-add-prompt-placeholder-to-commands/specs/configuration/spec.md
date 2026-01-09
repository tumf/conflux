# configuration Specification Delta

## ADDED Requirements

### Requirement: System Prompt for Apply and Archive Commands

The orchestrator SHALL support `{prompt}` placeholder in both `apply_command` and `archive_command` templates, allowing system-provided instructions to be injected into agent commands.

The orchestrator SHALL provide `apply_prompt` and `archive_prompt` configuration options to define default prompt values.

#### Scenario: Apply command with prompt placeholder

- **GIVEN** `apply_command` is configured as `"agent apply {change_id} {prompt}"`
- **AND** `apply_prompt` is configured as `"Remove out-of-scope tasks."`
- **WHEN** applying change `add-feature`
- **THEN** the executed command is `agent apply add-feature Remove out-of-scope tasks.`

#### Scenario: Apply command uses default prompt when not configured

- **GIVEN** `apply_command` is configured as `"agent apply {change_id} {prompt}"`
- **AND** `apply_prompt` is NOT configured
- **WHEN** applying change `add-feature`
- **THEN** the default apply prompt value is used for `{prompt}` expansion

#### Scenario: Archive command with empty prompt

- **GIVEN** `archive_command` is configured as `"agent archive {change_id} {prompt}"`
- **AND** `archive_prompt` is NOT configured (defaults to empty string)
- **WHEN** archiving change `add-feature`
- **THEN** the executed command is `agent archive add-feature ` (prompt expanded to empty)

#### Scenario: Custom archive prompt

- **GIVEN** `archive_command` is configured as `"agent archive {change_id} {prompt}"`
- **AND** `archive_prompt` is configured as `"Verify all tests pass."`
- **WHEN** archiving change `add-feature`
- **THEN** the executed command is `agent archive add-feature Verify all tests pass.`

#### Scenario: Backward compatibility without prompt placeholder

- **GIVEN** `apply_command` is configured as `"agent apply {change_id}"` (no `{prompt}`)
- **WHEN** applying change `add-feature`
- **THEN** the executed command is `agent apply add-feature`
- **AND** no errors occur (prompt is simply not expanded)

## MODIFIED Requirements

### Requirement: Placeholder Expansion (MODIFIED)

Command templates SHALL support the following placeholders:
- `{change_id}` - The change ID being processed (used in apply_command, archive_command)
- `{prompt}` - System-provided instructions (used in apply_command, archive_command, analyze_command)

#### Scenario: Both placeholders in apply command

- **WHEN** `apply_command` is `"agent --id {change_id} --instructions '{prompt}'"`
- **AND** change ID is `fix-bug`
- **AND** apply prompt is `"Focus on core changes"`
- **THEN** the executed command is `agent --id fix-bug --instructions 'Focus on core changes'`

#### Scenario: Multiple {prompt} placeholders

- **WHEN** `apply_command` is `"agent apply {change_id} --pre '{prompt}' --post '{prompt}'"`
- **AND** change ID is `fix-bug`
- **AND** apply prompt is `"Be careful"`
- **THEN** the executed command is `agent apply fix-bug --pre 'Be careful' --post 'Be careful'`
