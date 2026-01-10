# cli Specification Delta

## ADDED Requirements

### Requirement: Apply Context History

The orchestrator MUST capture the agent's final summary message from each apply attempt and include it in subsequent apply prompts for the same change.

#### Scenario: First apply attempt has no history

- **WHEN** the orchestrator executes apply for a change for the first time
- **THEN** the prompt contains only the base apply_prompt from configuration
- **AND** no `<last_apply>` tags are included

#### Scenario: Second apply includes previous attempt summary

- **WHEN** the orchestrator executes apply for a change for the second time
- **AND** the first attempt returned a summary message from the agent
- **THEN** the prompt contains the base apply_prompt
- **AND** the prompt contains a `<last_apply attempt="1">` block
- **AND** the block contains the agent's summary message from the first attempt

#### Scenario: Multiple previous attempts are included

- **WHEN** the orchestrator executes apply for a change for the third time
- **THEN** the prompt contains `<last_apply attempt="1">` and `<last_apply attempt="2">` blocks
- **AND** blocks are ordered by attempt number (oldest first)
- **AND** each block contains the agent's summary message from that attempt

#### Scenario: History is cleared on archive

- **WHEN** a change is successfully archived
- **THEN** the apply history for that change is cleared from memory
- **AND** subsequent apply attempts for the same change_id (if unarchived) start fresh

### Requirement: Apply History Context Format

The apply history context MUST be formatted as XML-like tags containing the agent's summary message.

#### Scenario: Context format structure

- **GIVEN** a previous apply attempt where the agent returned the summary:
  "Implemented task 1.1 and 1.2. Found issue with type conversion in auth.rs:42 that needs fixing."
- **WHEN** the context is formatted for the next prompt
- **THEN** the output is:
  ```
  <last_apply attempt="1">
  Implemented task 1.1 and 1.2. Found issue with type conversion in auth.rs:42 that needs fixing.
  </last_apply>
  ```

#### Scenario: Context appended to base prompt

- **GIVEN** base apply_prompt is "スコープ外タスクは削除せよ"
- **AND** there is one previous attempt with agent summary "Task 1.1 completed."
- **WHEN** the full prompt is built
- **THEN** the prompt format is:
  ```
  スコープ外タスクは削除せよ

  <last_apply attempt="1">
  Task 1.1 completed.
  </last_apply>
  ```

#### Scenario: Agent summary captured from apply response

- **WHEN** the openspec:apply skill completes execution
- **THEN** the agent returns a summary message describing work done
- **AND** the orchestrator captures this summary message for history
