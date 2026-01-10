# configuration Specification Delta

## ADDED Requirements

### Requirement: Max Iterations Configuration

The orchestrator SHALL support a configurable maximum iteration limit to prevent infinite loops.

#### Scenario: Configure max iterations in config file

- **GIVEN** `.openspec-orchestrator.jsonc` contains:
  ```jsonc
  {
    "max_iterations": 100
  }
  ```
- **WHEN** the orchestrator runs
- **THEN** the loop stops after 100 iterations
- **AND** the finish status is `iteration_limit`
- **AND** a log message indicates "Max iterations (100) reached"

#### Scenario: Default limit when not configured

- **GIVEN** `max_iterations` is not set in config
- **WHEN** the orchestrator runs
- **THEN** the default limit of 50 iterations is applied
- **AND** the loop stops after 50 iterations if not complete

#### Scenario: CLI flag overrides config

- **GIVEN** config file has `"max_iterations": 100`
- **WHEN** user runs `openspec-orchestrator run --max-iterations 50`
- **THEN** the loop stops after 50 iterations
- **AND** CLI value takes precedence over config file

#### Scenario: Zero disables limit

- **GIVEN** `max_iterations` is set to `0`
- **WHEN** the orchestrator runs
- **THEN** no iteration limit is applied
- **AND** the loop continues until all changes complete or error occurs

#### Scenario: Warning when approaching limit

- **GIVEN** `max_iterations` is set to `100`
- **WHEN** iteration count reaches 80 (80% of limit)
- **THEN** a warning log is emitted: "Approaching max iterations: 80/100"

### Requirement: Iteration Limit Finish Status

The `on_finish` hook SHALL receive `iteration_limit` status when the loop stops due to reaching max iterations.

#### Scenario: Hook receives iteration_limit status

- **GIVEN** `max_iterations` is set to `10`
- **AND** `on_finish` hook is configured
- **WHEN** the loop reaches iteration 10
- **THEN** `on_finish` hook is called with `{status}` = `iteration_limit`
- **AND** `{iteration}` = `10`
